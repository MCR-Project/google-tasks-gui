pub mod api;
pub mod auth;
pub mod db;
pub mod sync;

pub use api::{GoogleTasksClient, TaskList, TaskLocal};
pub use db::Database;
pub use sync::{SyncCommand, SyncEvent, SyncManager};
use futures::future::join_all;
use std::error::Error;

/// Resolves authentication by checking Keyring first, falling back to OAuth PKCE.
pub async fn obtain_authenticated_client() -> Result<GoogleTasksClient, Box<dyn Error + Send + Sync>>
{
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

/// Fetches task lists and tasks from Google API in parallel with optional delta timestamp caching into SQLite.
pub async fn sync_remote_to_db_delta(
    client: &mut GoogleTasksClient,
    db: &mut Database,
    last_sync: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let lists = client.get_task_lists().await?;
    db.save_task_lists(&lists)?;

    let fetch_futures = lists.iter().map(|list| {
        let mut client_clone = client.clone();
        let list_id = list.id.clone();
        async move {
            let raw_tasks = client_clone
                .get_tasks(&list_id, true, last_sync.as_ref())
                .await?;
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

pub async fn sync_remote_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    sync_remote_to_db_delta(client, db, None).await
}

pub async fn sync_local_to_db(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // 0. Sync pending local task lists created offline/locally
    if let Ok(dirty_lists) = db.get_dirty_task_lists() {
        for list in dirty_lists {
            match client.create_task_list(&list.title).await {
                Ok(created_list) => {
                    println!(
                        "📋 Created remote Google Task List '{}' with ID: {}",
                        created_list.title, created_list.id
                    );
                    if let Err(err) = db.migrate_local_list_id(&list.id, &created_list) {
                        eprintln!("Failed to migrate local list ID {}: {}", list.id, err);
                    }
                }
                Err(err) => {
                    eprintln!("Error creating remote task list {}: {}", list.title, err);
                }
            }
        }
    }

    // 1. Process pending soft deletions
    if let Ok(pending_deletions) = db.get_pending_deletions() {
        for task in pending_deletions {
            if !task.id.is_empty()
                && !task.id.starts_with("local_")
                && !task.list_id.starts_with("list_")
            {
                if let Err(err) = client.delete_task(&task.list_id, &task.id).await {
                    eprintln!("Error deleting remote task {}: {}", task.id, err);
                    continue;
                }
            }
            let _ = db.purge_task(&task.id);
        }
    }

    // 2. Process dirty creations and updates
    let dirty_tasks = db.get_dirty_task()?;

    if dirty_tasks.is_empty() {
        return Ok(());
    }

    for mut task in dirty_tasks {
        if task.list_id.starts_with("list_") {
            continue;
        }

        if task.id.is_empty() || task.id.starts_with("local_") {
            let temp_id = task.id.clone();
            let raw_task = client.create_task(&task.list_id, &task).await?;
            if !temp_id.is_empty() {
                let _ = db.purge_task(&temp_id);
            }
            task = TaskLocal::from_task_get(raw_task, task.list_id.clone());
            task.is_dirty = false;
            db.save_tasks(&[task])?;
        } else {
            let raw_task = client
                .update_task(
                    &task.list_id,
                    &task.id,
                    task.title.as_deref(),
                    task.notes.as_deref(),
                    Some(task.is_completed),
                    task.due.as_ref(),
                )
                .await?;
            task = TaskLocal::from_task_get(raw_task, task.list_id.clone());
            task.is_dirty = false;
            db.save_tasks(&[task])?;
        }
    }
    Ok(())
}
