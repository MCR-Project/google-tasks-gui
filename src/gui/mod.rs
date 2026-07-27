use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::api::{GoogleTasksClient, TaskLocal};
use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub client: Arc<Mutex<GoogleTasksClient>>,
    pub db: Database,
}

#[derive(Deserialize)]
pub struct CreateListPayload {
    pub title: String,
}

#[derive(Deserialize)]
pub struct CreateTaskPayload {
    pub list_id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateTaskPayload {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due: Option<String>,
    pub is_completed: Option<bool>,
}

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Runs the Axum Web Server and opens the dedicated Libadwaita-styled App Window
pub async fn run(
    client: GoogleTasksClient,
    db: Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let state = AppState {
        client: Arc::new(Mutex::new(client)),
        db,
    };

    let app = Router::new()
        .route("/api/lists", get(get_lists).post(create_list))
        .route("/api/lists/:id/tasks", get(get_tasks))
        .route("/api/tasks", post(create_task))
        .route(
            "/api/tasks/:id",
            axum::routing::patch(update_task).delete(delete_task),
        )
        .route("/api/sync", post(sync_data))
        .fallback_service(ServeDir::new("web"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("🚀 Google Tasks Desktop App running on http://{}", addr);

    let url = format!("http://{}", addr);
    open_standalone_app_window(&url);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Launches a dedicated desktop app window without address bar or browser tabs
fn open_standalone_app_window(url: &str) {
    let app_arg = format!("--app={}", url);
    let name_arg = "--name=GoogleTasks";
    let class_arg = "--class=GoogleTasks";

    // Attempt launching via Chrome app mode for a native Libadwaita window feel
    if Command::new("google-chrome")
        .args([&app_arg, name_arg, class_arg])
        .spawn()
        .is_err()
    {
        if Command::new("chromium")
            .args([&app_arg, name_arg, class_arg])
            .spawn()
            .is_err()
        {
            let _ = open::that(url);
        }
    }
}

async fn get_lists(
    State(state): State<AppState>,
) -> (StatusCode, Json<ApiResponse<Vec<crate::api::TaskList>>>) {
    match state.db.get_task_lists() {
        Ok(lists) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(lists),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn create_list(
    State(state): State<AppState>,
    Json(payload): Json<CreateListPayload>,
) -> (StatusCode, Json<ApiResponse<crate::api::TaskList>>) {
    let mut client = state.client.lock().await;

    match client.create_task_list(&payload.title).await {
        Ok(new_list) => {
            let _ = state.db.save_task_lists(&[new_list.clone()]);
            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(new_list),
                    error: None,
                }),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn get_tasks(
    State(state): State<AppState>,
    Path(list_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Vec<TaskLocal>>>) {
    match state.db.get_tasks_for_list(&list_id) {
        Ok(tasks) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(tasks),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskPayload>,
) -> (StatusCode, Json<ApiResponse<TaskLocal>>) {
    let due_dt = payload.due.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
            .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let new_task = TaskLocal {
        id: String::new(),
        list_id: payload.list_id,
        title: Some(payload.title),
        is_completed: false,
        notes: payload.notes,
        due: due_dt,
        completed: None,
        parent: None,
        updated: Some(chrono::Utc::now()),
        is_dirty: true,
    };

    if let Err(e) = state.db.save_tasks(&[new_task.clone()]) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(new_task),
            error: None,
        }),
    )
}

async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(payload): Json<UpdateTaskPayload>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    let lists = match state.db.get_task_lists() {
        Ok(l) => l,
        Err(_) => Vec::new(),
    };

    for list in lists {
        if let Ok(mut tasks) = state.db.get_tasks_for_list(&list.id) {
            if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
                if let Some(t) = payload.title {
                    task.title = Some(t);
                }
                if let Some(n) = payload.notes {
                    task.notes = Some(n);
                }
                if let Some(d) = payload.due {
                    task.due = chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                        .ok()
                        .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
                        .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc));
                }
                if let Some(c) = payload.is_completed {
                    task.is_completed = c;
                    if c {
                        task.completed = Some(chrono::Utc::now());
                    } else {
                        task.completed = None;
                    }
                }
                task.updated = Some(chrono::Utc::now());
                task.is_dirty = true;

                let _ = state.db.save_tasks(&[task.clone()]);
                return (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        data: Some(true),
                        error: None,
                    }),
                );
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Task not found".to_string()),
        }),
    )
}

async fn delete_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<bool>>) {
    match state.db.delete_tasks_db(&[task_id]) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(true),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

async fn sync_data(
    State(state): State<AppState>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let mut client = state.client.lock().await;
    let mut db = state.db.clone();

    let _ = crate::sync_local_to_db(&mut client, &mut db).await;
    let _ = crate::sync_remote_to_db(&mut client, &mut db).await;

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some("Synced successfully!".to_string()),
            error: None,
        }),
    )
}
