mod api;
mod auth;
mod db;
mod ui;

use api::{GoogleTasksClient, TaskLocal};
use db::Database;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    println!("🚀 Starting gTasks TUI Application...\n");

    // Step 1: Authenticate and obtain API Client
    let mut client = obtain_authenticated_client().await?;

    // Step 2: Initialize local SQLite Database
    let mut db = Database::new("task_lists.db")?;

    // Step 3: Synchronize remote & local data on startup
    println!("🔄 Syncing data with Google Tasks API...");
    let _ = sync_remote_to_db(&mut client, &mut db).await;
    let _ = sync_local_to_db(&mut client, &mut db).await;

    // Step 4: Run the interactive Terminal UI Dashboard!
    ui::run(&mut client, &mut db).await?;

    println!("\n✨ Thank you for using gTasks!");
    Ok(())
}

/// Resolves authentication by checking Keyring first, falling back to OAuth PKCE.
async fn obtain_authenticated_client() -> Result<GoogleTasksClient, Box<dyn Error>> {
    let token_response = if let Ok(refresh_token) = auth::keyring::get_refresh_token() {
        println!("🔐 Found saved refresh token in OS keyring. Refreshing access token...");
        auth::refresh_access_token(&refresh_token).await?
    } else {
        println!("🔐 No refresh token found. Starting browser OAuth authentication...");
        let token = auth::authenticate().await?;
        if let Some(ref refresh_token) = token.refresh_token {
            auth::keyring::save_refresh_token(refresh_token)?;
            println!("💾 Saved new refresh token to OS Keyring.");
        }
        token
    };

    println!(
        "✅ Authentication successful (Token expires in {}s)\n",
        token_response.expires_in
    );

    Ok(GoogleTasksClient::new(token_response.access_token))
}

/// Fetches task lists and tasks from Google API and caches them into SQLite.
pub async fn sync_remote_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error>> {
    let lists = client.get_task_lists().await?;
    println!("📋 Retrieved {} Task List(s)", lists.len());

    db.save_task_lists(&lists)?;

    for list in &lists {
        let raw_tasks = client.get_tasks(&list.id, true).await?;
        let local_tasks: Vec<TaskLocal> = raw_tasks
            .into_iter()
            .map(|raw| TaskLocal::from_task_get(raw, list.id.clone()))
            .collect();

        db.save_tasks(&local_tasks)?;
        println!(
            "  • Saved {} task(s) for list '{}'",
            local_tasks.len(),
            list.title
        );
    }

    Ok(())
}

pub async fn sync_local_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error>> {
    let dirty_tasks = db.get_dirty_task()?;

    if dirty_tasks.is_empty() {
        println!("✅ No dirty tasks to sync.");
        return Ok(());
    }
    println!("📋 Retrieved {} Dirty Task(s)", dirty_tasks.len());

    for mut task in dirty_tasks {
        let raw_tasks = client
            .update_task(
                &task.list_id,
                &task.id,
                task.title.as_deref(),
                task.notes.as_deref(),
                Some(task.is_completed),
                task.due.as_ref(),
            )
            .await?;

        task = TaskLocal::from_task_get(raw_tasks, task.list_id.clone());
        task.is_dirty = false;
        db.save_tasks(&[task])?;
    }
    Ok(())
}
