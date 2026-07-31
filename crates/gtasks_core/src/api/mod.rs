use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskList {
    pub id: String,
    pub title: String,
    pub updated: Option<String>,
}

// Wrapper for the Google Tasks API client response
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskListsResponse {
    pub items: Option<Vec<TaskList>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskGet {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>,    // Task done or not
    pub notes: Option<String>,     // description of the task
    pub due: Option<String>,       // Deadline
    pub completed: Option<String>, // Completion date of the task
    pub parent: Option<String>,    // Parent task ID (in case of subtask)
    pub updated: Option<String>,   // Last modification date
    pub deleted: Option<bool>,     // Remote deletion status
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskLocal {
    pub id: String,
    pub list_id: String,
    pub title: Option<String>,
    pub is_completed: bool,                               // Task done or not
    pub notes: Option<String>,                            // description of the task
    pub due: Option<chrono::DateTime<chrono::Utc>>,       // Deadline
    pub completed: Option<chrono::DateTime<chrono::Utc>>, // Completion date of the task
    pub parent: Option<String>,                           // Parent task ID (in case of subtask)
    pub updated: Option<chrono::DateTime<chrono::Utc>>,   // Last modification date
    pub is_dirty: bool,
    pub is_deleted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TasksResponse {
    pub items: Option<Vec<TaskGet>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

/// Patch payload builder for updating task fields.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaskPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

impl TaskPatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn completed(mut self, completed: bool) -> Self {
        self.status = Some(if completed {
            "completed".to_string()
        } else {
            "needsAction".to_string()
        });
        self
    }

    pub fn due(mut self, due: &chrono::DateTime<chrono::Utc>) -> Self {
        self.due = Some(due.to_rfc3339());
        self
    }
}

// API
#[derive(Debug, Clone)]
pub struct GoogleTasksClient {
    pub client: reqwest::Client,
    pub access_token: Arc<RwLock<String>>,
}

impl GoogleTasksClient {
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(access_token)),
        }
    }

    pub async fn get_access_token(&self) -> String {
        self.access_token.read().await.clone()
    }

    // Get the list of task lists for the authenticated user
    pub async fn get_task_lists(&self) -> crate::Result<Vec<TaskList>> {
        let url = "https://www.googleapis.com/tasks/v1/users/@me/lists";
        let mut all_lists = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let response = self
                .execute_with_retry(|client, token| {
                    let mut req = client.get(url).bearer_auth(token);
                    if let Some(ref pt) = page_token {
                        req = req.query(&[("pageToken", pt)]);
                    }
                    req
                })
                .await?;
            let response = response.error_for_status()?;
            let task_lists_response: TaskListsResponse = response.json().await?;

            if let Some(items) = task_lists_response.items {
                all_lists.extend(items);
            }

            match task_lists_response.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(all_lists)
    }

    // Create a new task list
    pub async fn create_task_list(&self, title: &str) -> crate::Result<TaskList> {
        let url = "https://www.googleapis.com/tasks/v1/users/@me/lists";
        let payload = serde_json::json!({ "title": title });
        let response = self
            .execute_with_retry(|client, token| client.post(url).bearer_auth(token).json(&payload))
            .await?;
        let response = response.error_for_status()?;
        let created: TaskList = response.json().await?;
        Ok(created)
    }

    pub async fn delete_task_list(&self, list_id: &str) -> crate::Result<()> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/users/@me/lists/{}",
            list_id
        );
        let response = self
            .execute_with_retry(|client, token| client.delete(&url).bearer_auth(token))
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND
            || response.status() == reqwest::StatusCode::NO_CONTENT
        {
            return Ok(());
        }
        response.error_for_status()?;
        Ok(())
    }

    // Get the tasks for a specific task list
    pub async fn get_tasks(
        &self,
        list_id: &str,
        show_completed: bool,
        updated_min: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> crate::Result<Vec<TaskGet>> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{}/tasks",
            list_id
        );

        let updated_min_str = updated_min.map(|dt| dt.to_rfc3339());
        let mut all_tasks = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let response = self
                .execute_with_retry(|client, token| {
                    let mut query = vec![
                        ("showCompleted", show_completed.to_string()),
                        ("showHidden", show_completed.to_string()),
                        ("showDeleted", "true".to_string()),
                    ];
                    if let Some(ref dt_str) = updated_min_str {
                        query.push(("updatedMin", dt_str.clone()));
                    }
                    if let Some(ref pt) = page_token {
                        query.push(("pageToken", pt.clone()));
                    }
                    client.get(&url).bearer_auth(token).query(&query)
                })
                .await?;
            let response = response.error_for_status()?;
            let tasks_response: TasksResponse = response.json().await?;

            if let Some(items) = tasks_response.items {
                all_tasks.extend(items);
            }

            match tasks_response.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(all_tasks)
    }

    pub async fn create_task(
        &self,
        list_id: &str,
        task: &TaskLocal,
    ) -> crate::Result<TaskGet> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{}/tasks",
            list_id
        );
        let body = serde_json::json!({
            "title": task.title,
            "notes": task.notes,
            "due": task.due.map(|d| d.to_rfc3339()),
            "parent": task.parent,
        });

        let response = self
            .execute_with_retry(|client, token| client.post(&url).bearer_auth(token).json(&body))
            .await?;
        let response = response.error_for_status()?;
        let created_task: TaskGet = response.json().await?;
        Ok(created_task)
    }

    pub async fn patch_task(
        &self,
        list_id: &str,
        task_id: &str,
        patch: &TaskPatch,
    ) -> crate::Result<TaskGet> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}"
        );
        let response = self
            .execute_with_retry(|client, token| client.patch(&url).bearer_auth(token).json(patch))
            .await?;
        let response = response.error_for_status()?;
        let updated_task: TaskGet = response.json().await?;
        Ok(updated_task)
    }

    pub async fn update_task(
        &self,
        list_id: &str,
        task_id: &str,
        title: Option<&str>,
        notes: Option<&str>,
        completed: Option<bool>,
        due: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> crate::Result<TaskGet> {
        let mut patch = TaskPatch::new();
        if let Some(t) = title {
            patch = patch.title(t);
        }
        if let Some(n) = notes {
            patch = patch.notes(n);
        }
        if let Some(c) = completed {
            patch = patch.completed(c);
        }
        if let Some(d) = due {
            patch = patch.due(d);
        }
        self.patch_task(list_id, task_id, &patch).await
    }

    pub async fn toggle_task_completion(
        &self,
        list_id: &str,
        task_id: &str,
        completed: bool,
    ) -> crate::Result<TaskGet> {
        self.update_task(list_id, task_id, None, None, Some(completed), None)
            .await
    }

    async fn execute_with_retry(
        &self,
        build_request: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
    ) -> crate::Result<reqwest::Response> {
        let token = self.access_token.read().await.clone();
        let request = build_request(&self.client, &token);
        let response = request.send().await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Ok(refresh_token) = crate::auth::keyring::get_refresh_token() {
                if let Ok(token_response) = crate::auth::refresh_access_token(&refresh_token).await
                {
                    let mut token_writer = self.access_token.write().await;
                    *token_writer = token_response.access_token.clone();

                    let retry_token = build_request(&self.client, &token_response.access_token);
                    return Ok(retry_token.send().await?);
                }
            }
        }
        Ok(response)
    }

    pub async fn delete_task(
        &self,
        list_id: &str,
        task_id: &str,
    ) -> crate::Result<()> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}"
        );

        let response = self
            .execute_with_retry(|client, token| client.delete(&url).bearer_auth(token))
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        response.error_for_status()?;
        Ok(())
    }
}

impl TaskLocal {
    pub fn from_task_get(task_get: TaskGet, list_id: String) -> Self {
        let due = task_get.due.as_ref().and_then(|due_str| {
            DateTime::parse_from_rfc3339(due_str)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let completed = task_get.completed.as_ref().and_then(|completed_str| {
            DateTime::parse_from_rfc3339(completed_str)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let updated = task_get.updated.as_ref().and_then(|updated_str| {
            DateTime::parse_from_rfc3339(updated_str)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });
        let is_dirty = false;
        let is_deleted = task_get.deleted.unwrap_or(false);

        TaskLocal {
            id: task_get.id,
            list_id,
            title: task_get.title,
            is_completed: task_get.status.map(|s| s == "completed").unwrap_or(false),
            notes: task_get.notes,
            due,
            completed,
            parent: task_get.parent,
            updated,
            is_dirty,
            is_deleted,
        }
    }

    pub fn is_local_id(&self) -> bool {
        TaskId::new(&self.id).is_local()
    }

    pub fn is_local_list(&self) -> bool {
        TaskListId::new(&self.list_id).is_local()
    }

    pub fn toggle_task_completion(&self) -> TaskLocal {
        let mut updated_task = self.clone();
        updated_task.is_completed = !self.is_completed;
        if updated_task.is_completed {
            updated_task.completed = Some(Utc::now());
        } else {
            updated_task.completed = None;
        }
        updated_task
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    NeedsAction,
    Completed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::NeedsAction => "needsAction",
            TaskStatus::Completed => "completed",
        }
    }
}

impl From<&str> for TaskStatus {
    fn from(s: &str) -> Self {
        match s {
            "completed" => TaskStatus::Completed,
            _ => TaskStatus::NeedsAction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn is_local(&self) -> bool {
        self.0.is_empty() || self.0.starts_with("local_")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskListId(pub String);

impl TaskListId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn is_local(&self) -> bool {
        self.0.starts_with("list_")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TaskListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[allow(async_fn_in_trait)]
pub trait TasksApi: Send + Sync {

    async fn get_task_lists(&self) -> crate::Result<Vec<TaskList>>;
    async fn create_task_list(&self, title: &str) -> crate::Result<TaskList>;
    async fn delete_task_list(&self, list_id: &str) -> crate::Result<()>;
    async fn get_tasks(
        &self,
        list_id: &str,
        show_completed: bool,
        updated_min: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> crate::Result<Vec<TaskGet>>;
    async fn create_task(&self, list_id: &str, task: &TaskLocal) -> crate::Result<TaskGet>;
    async fn patch_task(&self, list_id: &str, task_id: &str, patch: &TaskPatch) -> crate::Result<TaskGet>;
    async fn delete_task(&self, list_id: &str, task_id: &str) -> crate::Result<()>;
}

impl TasksApi for GoogleTasksClient {
    async fn get_task_lists(&self) -> crate::Result<Vec<TaskList>> {
        self.get_task_lists().await
    }
    async fn create_task_list(&self, title: &str) -> crate::Result<TaskList> {
        self.create_task_list(title).await
    }
    async fn delete_task_list(&self, list_id: &str) -> crate::Result<()> {
        self.delete_task_list(list_id).await
    }
    async fn get_tasks(
        &self,
        list_id: &str,
        show_completed: bool,
        updated_min: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> crate::Result<Vec<TaskGet>> {
        self.get_tasks(list_id, show_completed, updated_min).await
    }
    async fn create_task(&self, list_id: &str, task: &TaskLocal) -> crate::Result<TaskGet> {
        self.create_task(list_id, task).await
    }
    async fn patch_task(&self, list_id: &str, task_id: &str, patch: &TaskPatch) -> crate::Result<TaskGet> {
        self.patch_task(list_id, task_id, patch).await
    }
    async fn delete_task(&self, list_id: &str, task_id: &str) -> crate::Result<()> {
        self.delete_task(list_id, task_id).await
    }
}


