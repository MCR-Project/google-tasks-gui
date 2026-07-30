pub mod api;
pub mod auth;
pub mod db;
pub mod error;
pub mod sync;
pub mod util;

pub use api::{GoogleTasksClient, TaskList, TaskLocal};
pub use db::Database;
pub use error::GTasksError;
pub use sync::{SyncCommand, SyncEvent, SyncManager};
pub use util::{order_tasks_hierarchically, parse_nlp_task};
use futures::future::join_all;
use std::error::Error;

/// Resolves authentication by checking Keyring first, falling back to OAuth PKCE.
pub async fn obtain_authenticated_client() -> Result<GoogleTasksClient, Box<dyn Error + Send + Sync>>
{
    let token_response = if let Ok(refresh_token) = auth::keyring::get_refresh_token() {
        tracing::info!("🔐 Found saved refresh token in OS keyring. Refreshing access token...");
        auth::refresh_access_token(&refresh_token).await?
    } else {
        tracing::info!("🔐 No refresh token found. Starting browser OAuth authentication...");
        let token = auth::authenticate().await?;
        if let Some(ref refresh_token) = token.refresh_token {
            auth::keyring::save_refresh_token(refresh_token)?;
            tracing::info!("💾 Saved new refresh token to OS Keyring.");
        }
        token
    };

    tracing::info!(
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
    let db_clone = db.clone();
    let lists_clone = lists.clone();
    tokio::task::spawn_blocking(move || db_clone.save_task_lists(&lists_clone)).await??;

    let fetch_futures = lists.iter().map(|list| {
        let mut client_clone = client.clone();
        let list_id = list.id.clone();
        async move {
            let raw_tasks = client_clone
                .get_tasks(&list_id, true, last_sync.as_ref())
                .await?;
            Ok::<(String, Vec<api::TaskGet>), Box<dyn Error + Send + Sync>>((list_id, raw_tasks))
        }
    });

    let results = join_all(fetch_futures).await;

    for res in results.into_iter().flatten() {
        let (list_id, raw_tasks) = res;
        let mut tasks_to_save = Vec::new();
        for raw in raw_tasks {
            if raw.deleted == Some(true) {
                let db_clone = db.clone();
                let raw_id = raw.id.clone();
                let target_id = raw_id.clone();
                if let Ok(Err(err)) = tokio::task::spawn_blocking(move || db_clone.purge_task(&target_id)).await {
                    tracing::warn!("Failed to purge deleted task {}: {}", raw_id, err);
                }
            } else {
                tasks_to_save.push(TaskLocal::from_task_get(raw, list_id.clone()));
            }
        }
        if !tasks_to_save.is_empty() {
            let db_clone = db.clone();
            tokio::task::spawn_blocking(move || db_clone.save_tasks(&tasks_to_save)).await??;
        }
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
    let db_clone = db.clone();
    if let Ok(dirty_lists) = tokio::task::spawn_blocking(move || db_clone.get_dirty_task_lists()).await? {
        for list in dirty_lists {
            match client.create_task_list(&list.title).await {
                Ok(created_list) => {
                    tracing::info!(
                        "📋 Created remote Google Task List '{}' with ID: {}",
                        created_list.title, created_list.id
                    );
                    let db_clone = db.clone();
                    let list_id = list.id.clone();
                    let cl = created_list.clone();
                    if let Ok(Err(err)) = tokio::task::spawn_blocking(move || db_clone.migrate_local_list_id(&list_id, &cl)).await {
                        tracing::error!("Failed to migrate local list ID {}: {}", list.id, err);
                    }
                }
                Err(err) => {
                    tracing::error!("Error creating remote task list {}: {}", list.title, err);
                }
            }
        }
    }

    // 1. Process pending soft deletions
    let db_clone = db.clone();
    if let Ok(pending_deletions) = tokio::task::spawn_blocking(move || db_clone.get_pending_deletions()).await? {
        for task in pending_deletions {
            if !task.id.is_empty()
                && !task.id.starts_with("local_")
                && !task.list_id.starts_with("list_")
            {
                if let Err(err) = client.delete_task(&task.list_id, &task.id).await {
                    tracing::error!("Error deleting remote task {}: {}", task.id, err);
                    continue;
                }
            }
            let db_clone = db.clone();
            let task_id = task.id.clone();
            let target_id = task_id.clone();
            if let Ok(Err(err)) = tokio::task::spawn_blocking(move || db_clone.purge_task(&target_id)).await {
                tracing::warn!("Failed to purge deleted task {}: {}", task_id, err);
            }
        }
    }

    // 2. Process dirty creations and updates
    let db_clone = db.clone();
    let dirty_tasks = tokio::task::spawn_blocking(move || db_clone.get_dirty_task()).await??;

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
            let db_clone = db.clone();
            let temp_id_clone = temp_id.clone();
            let target_id = temp_id_clone.clone();
            if let Ok(Err(err)) = tokio::task::spawn_blocking(move || db_clone.purge_task(&target_id)).await {
                tracing::warn!("Failed to purge temporary local task {}: {}", temp_id_clone, err);
            }
            task = TaskLocal::from_task_get(raw_task, task.list_id.clone());
            task.is_dirty = false;
            let db_clone = db.clone();
            let task_clone = task.clone();
            tokio::task::spawn_blocking(move || db_clone.save_tasks(&[task_clone])).await??;
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
            let db_clone = db.clone();
            let task_id = task.id.clone();
            let target_id = task_id.clone();
            if let Ok(Err(err)) = tokio::task::spawn_blocking(move || db_clone.mark_task_clean(&target_id)).await {
                tracing::warn!("Failed to mark task clean {}: {}", task_id, err);
            }
            task = TaskLocal::from_task_get(raw_task, task.list_id.clone());
            task.is_dirty = false;
            let db_clone = db.clone();
            let task_clone = task.clone();
            tokio::task::spawn_blocking(move || db_clone.save_tasks(&[task_clone])).await??;
        }
    }
    Ok(())
}
