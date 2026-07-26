mod api;
mod auth;
mod db;

use api::{GoogleTasksClient, TaskLocal};
use db::Database;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    println!("🚀 Starting gTasks Headless Test Runner...\n");

    // Step 1: Authenticate and obtain API Client
    let mut client = obtain_authenticated_client().await?;

    // Step 2: Initialize local SQLite Database
    let mut db = Database::new("task_lists.db")?;

    // Step 3: Synchronize remote Google Tasks data into SQLite
    println!("🔄 Syncing data from Google Tasks API...");
    sync_remote_to_db(&mut client, &mut db).await?;
    sync_local_to_db(&mut client, &mut db).await?;

    // Step 4: Run API feature tests (create task & toggle completion)
    println!("\n🧪 Running API feature tests...");
    if let Err(err) = run_feature_tests(&mut client, &mut db).await {
        eprintln!("⚠️ Feature tests warning: {}", err);
    }

    // Step 5: Read and display stored SQLite state
    println!("\n💾 Local Database Contents:");
    display_database_contents(&db)?;

    println!("\n✨ Execution completed successfully!");
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
async fn sync_remote_to_db(
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

async fn sync_local_to_db(
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

/// Test runner for API actions: Task Creation and Task Completion Toggle.
async fn run_feature_tests(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error>> {
    let lists = db.get_task_lists()?;
    let first_list = match lists.first() {
        Some(l) => l,
        None => return Ok(()),
    };

    // 1. Test Task Creation
    let draft_task = TaskLocal {
        id: String::new(),
        list_id: first_list.id.clone(),
        title: Some("⚡ Refactored CLI Test Task".to_string()),
        is_completed: false,
        notes: Some("Testing task creation from refactored main.rs".to_string()),
        due: None,
        completed: None,
        parent: None,
        updated: None,
        is_dirty: true,
    };

    let created_raw = client.create_task(&first_list.id, &draft_task).await?;
    let created_task = TaskLocal::from_task_get(created_raw, first_list.id.clone());
    println!(
        "  ✅ [CREATE] Created task ID: '{}' | Title: '{}'",
        created_task.id,
        created_task.title.as_deref().unwrap_or("")
    );
    db.save_tasks(&[created_task.clone()])?;

    // 2. Test Task Completion Toggle (marking it completed)
    let toggled_raw = client
        .toggle_task_completion(&first_list.id, &created_task.id, true)
        .await?;
    let toggled_task = TaskLocal::from_task_get(toggled_raw, first_list.id.clone());
    println!(
        "  ✅ [TOGGLE] Task ID: '{}' status updated to: {}",
        toggled_task.id,
        if toggled_task.is_completed {
            "Completed ✅"
        } else {
            "Pending 🔲"
        }
    );
    db.save_tasks(&[toggled_task])?;

    let delete_result = client.delete_task(&first_list.id, &created_task.id).await;
    match delete_result {
        Ok(_) => {
            println!(
                "  ✅ [DELETE] Successfully deleted task ID: '{}'",
                created_task.id
            );
            db.delete_tasks_db(&[created_task.id])?;
        }
        Err(err) => {
            eprintln!(
                "⚠️ [DELETE] Failed to delete task ID: '{}'. Error: {}",
                created_task.id, err
            );
        }
    }

    Ok(())
}

/// Reads all records from SQLite and prints them in a clean tree format.
fn display_database_contents(db: &Database) -> Result<(), Box<dyn Error>> {
    let stored_task_lists = db.get_task_lists()?;

    for list in stored_task_lists {
        println!("  📁 [{}] {}", list.id, list.title);
        let stored_tasks = db.get_tasks_for_list(&list.id)?;
        for task in stored_tasks {
            let icon = if task.is_completed { "✅" } else { "🔲" };
            let title = task.title.as_deref().unwrap_or("(No Title)");
            println!("      {} [{}] {}", icon, task.id, title);
        }
    }

    Ok(())
}
