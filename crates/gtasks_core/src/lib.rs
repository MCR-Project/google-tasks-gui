pub mod api;
pub mod auth;
pub mod db;

pub use api::{GoogleTasksClient, TaskList, TaskLocal};
pub use db::Database;
use futures::future::join_all;
use std::error::Error;

/// Resolves authentication by checking Keyring first, falling back to OAuth PKCE.
pub async fn obtain_authenticated_client() -> Result<GoogleTasksClient, Box<dyn Error + Send + Sync>> {
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

/// Fetches task lists and tasks from Google API in parallel and caches them into SQLite.
pub async fn sync_remote_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let lists = client.get_task_lists().await?;
    db.save_task_lists(&lists)?;

    let fetch_futures = lists.iter().map(|list| {
        let mut client_clone = client.clone();
        let list_id = list.id.clone();
        async move {
            let raw_tasks = client_clone.get_tasks(&list_id, true).await?;
            let local_tasks: Vec<TaskLocal> = raw_tasks
                .into_iter()
                .map(|raw| TaskLocal::from_task_get(raw, list_id.clone()))
                .collect();
            Ok::<Vec<TaskLocal>, Box<dyn Error + Send + Sync>>(local_tasks)
        }
    });

    let results = join_all(fetch_futures).await;

    for local_tasks in results.into_iter().flatten() {
        db.save_tasks(&local_tasks)?;
    }

    Ok(())
}

pub async fn sync_local_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let dirty_tasks = db.get_dirty_task()?;

    if dirty_tasks.is_empty() {
        return Ok(());
    }

    for mut task in dirty_tasks {
        let raw_task = if task.id.is_empty() {
            // 🆕 Brand new task: Use HTTP POST (create_task)
            client.create_task(&task.list_id, &task).await?
        } else {
            // ✏️ Existing task edit: Use HTTP PATCH (update_task)
            client
                .update_task(
                    &task.list_id,
                    &task.id,
                    task.title.as_deref(),
                    task.notes.as_deref(),
                    Some(task.is_completed),
                    task.due.as_ref(),
                )
                .await?
        };

        // If it was a new task, delete the temporary empty-id record from SQLite
        if task.id.is_empty() {
            let _ = db.delete_tasks_db(&[String::new()]);
        }

        // Save official server task back to SQLite with is_dirty = false
        task = TaskLocal::from_task_get(raw_task, task.list_id.clone());
        task.is_dirty = false;
        db.save_tasks(&[task])?;
    }
    Ok(())
}
