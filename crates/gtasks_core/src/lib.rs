//! # gtasks_core
//!
//! `gtasks_core` is an offline-first, thread-safe Rust SDK for the **Google Tasks API**.
//!
//! It serves as the core synchronization engine and API client for task management applications,
//! featuring OAuth 2.0 PKCE authentication, secure OS keyring integration, local SQLite caching,
//! background delta synchronization, and natural language date parsing.
//!
//! ## Architectural Overview
//!
//! The crate is divided into five specialized modules:
//!
//! - [`api`]: Asynchronous Google Tasks REST API client with automatic token retries and pagination.
//! - [`auth`]: OAuth 2.0 Authorization Code Flow with PKCE and OS Keyring persistence (`keyring`).
//! - [`db`]: Embedded, thread-safe SQLite database manager (`rusqlite`) for offline task caching.
//! - [`sync`]: Background actor ([`SyncManager`]) driving periodic and window-focus synchronization.
//! - [`util`]: Helpers for natural language date parsing ([`parse_nlp_task`]) and tree hierarchies ([`order_tasks_hierarchically`]).
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use gtasks_core::{obtain_authenticated_client, Database, sync_remote_to_db};
//! use std::error::Error;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
//!     // 1. Authenticate (Keyring lookup with PKCE fallback)
//!     let mut client = obtain_authenticated_client().await?;
//!
//!     // 2. Open local SQLite cache
//!     let mut db = Database::new("tasks.db")?;
//!
//!     // 3. Bidirectionally sync changes between SQLite and Google Tasks API
//!     sync_remote_to_db(&mut client, &mut db).await?;
//!
//!     // 4. Query offline tasks
//!     let lists = db.get_task_lists()?;
//!     for list in lists {
//!         println!("Task List: {} (ID: {})", list.title, list.id);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## System Prerequisites
//!
//! On Linux targets, system secret service development headers are required for `keyring` compilation:
//! - Debian/Ubuntu: `libsecret-1-dev`, `libssl-dev`, `pkg-config`
//! - Fedora: `libsecret-devel`, `openssl-devel`
//! - Arch Linux: `libsecret`, `openssl`

pub mod api;
pub mod auth;
pub mod db;
pub mod error;
pub mod sync;
pub mod util;

pub use api::{GoogleTasksClient, TaskList, TaskLocal, TaskPatch};
pub use db::Database;
pub use error::{GTasksError, Result};
pub use sync::{SyncCommand, SyncEvent, SyncManager};
pub use util::{order_tasks_hierarchically, parse_nlp_task};
use futures::future::join_all;

/// Resolves authentication by checking Keyring first, falling back to OAuth PKCE.
pub async fn obtain_authenticated_client() -> crate::error::Result<GoogleTasksClient> {
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
    client: &GoogleTasksClient,
    db: &mut Database,
    last_sync: Option<chrono::DateTime<chrono::Utc>>,
) -> crate::error::Result<()> {
    let lists = client.get_task_lists().await?;
    let db_clone = db.clone();
    let lists_clone = lists.clone();
    tokio::task::spawn_blocking(move || db_clone.save_task_lists(&lists_clone)).await??;

    let fetch_futures = lists.iter().map(|list| {
        let client_clone = client.clone();
        let list_id = list.id.clone();
        async move {
            let raw_tasks = client_clone
                .get_tasks(&list_id, true, last_sync.as_ref())
                .await?;
            Ok::<(String, Vec<api::TaskGet>), crate::error::GTasksError>((list_id, raw_tasks))
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
    client: &GoogleTasksClient,
    db: &mut Database,
) -> crate::error::Result<()> {
    sync_remote_to_db_delta(client, db, None).await
}

pub async fn sync_local_to_db(
    client: &GoogleTasksClient,
    db: &mut Database,
) -> crate::error::Result<()> {


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
