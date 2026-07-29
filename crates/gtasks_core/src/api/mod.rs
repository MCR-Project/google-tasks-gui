use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskList {
    pub id: String,
    pub title: String,
    pub updated: Option<String>,
}

//Wrapper for the Google Tasks API client response
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskListsResponse {
    pub items: Option<Vec<TaskList>>,
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
}

// API
#[derive(Debug, Clone)]
pub struct GoogleTasksClient {
    pub client: reqwest::Client,
    pub access_token: String,
}

impl GoogleTasksClient {
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token,
        }
    }

    // Get the list of task lists for the authenticated user
    pub async fn get_task_lists(&mut self) -> Result<Vec<TaskList>, reqwest::Error> {
        let url = "https://www.googleapis.com/tasks/v1/users/@me/lists";
        let response = self
            .execute_with_retry(|client, token| client.get(url).bearer_auth(token))
            .await?;
        let response = response.error_for_status()?;

        let task_lists_response: TaskListsResponse = response.json().await?;
        Ok(task_lists_response.items.unwrap_or_default())
    }

    // Create a new task list
    pub async fn create_task_list(&mut self, title: &str) -> Result<TaskList, reqwest::Error> {
        let url = "https://www.googleapis.com/tasks/v1/users/@me/lists";
        let payload = serde_json::json!({ "title": title });
        let response = self
            .execute_with_retry(|client, token| client.post(url).bearer_auth(token).json(&payload))
            .await?;
        let response = response.error_for_status()?;
        let created: TaskList = response.json().await?;
        Ok(created)
    }

    pub async fn delete_task_list(&mut self, list_id: &str) -> Result<(), reqwest::Error> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/users/@me/lists/{}",
            list_id
        );
        let response = self
            .execute_with_retry(|client, token| client.delete(&url).bearer_auth(token))
            .await?;
        response.error_for_status()?;
        Ok(())
    }

    // Get the tasks for a specific task list
    pub async fn get_tasks(
        &mut self,
        list_id: &str,
        show_completed: bool,
    ) -> Result<Vec<TaskGet>, reqwest::Error> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{}/tasks",
            list_id
        );

        let response = self
            .execute_with_retry(|client, token| {
                client.get(&url).bearer_auth(token).query(&[
                    ("showCompleted", show_completed.to_string()),
                    ("showHidden", show_completed.to_string()),
                ])
            })
            .await?;
        let response = response.error_for_status()?;
        let tasks_response: TasksResponse = response.json().await?;
        Ok(tasks_response.items.unwrap_or_default())
    }

    pub async fn create_task(
        &mut self,
        list_id: &str,
        task: &TaskLocal,
    ) -> Result<TaskGet, reqwest::Error> {
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

    pub async fn update_task(
        &mut self,
        list_id: &str,
        task_id: &str,
        title: Option<&str>,
        notes: Option<&str>,
        completed: Option<bool>,
        due: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> Result<TaskGet, reqwest::Error> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}",
            list_id = list_id,
            task_id = task_id
        );

        let mut body = serde_json::Map::new();
        if let Some(title) = title {
            body.insert(
                "title".to_string(),
                serde_json::Value::String(title.to_string()),
            );
        }
        if let Some(notes) = notes {
            body.insert(
                "notes".to_string(),
                serde_json::Value::String(notes.to_string()),
            );
        }
        if let Some(due) = due {
            body.insert(
                "due".to_string(),
                serde_json::Value::String(due.to_rfc3339()),
            );
        }
        if let Some(completed) = completed {
            let status_str = if completed {
                "completed"
            } else {
                "needsAction"
            };
            body.insert(
                "status".to_string(),
                serde_json::Value::String(status_str.to_string()),
            );
        }

        let response = self
            .execute_with_retry(|client, token| client.patch(&url).bearer_auth(token).json(&body))
            .await?;
        let response = response.error_for_status()?;
        let toggled_task: TaskGet = response.json().await?;
        Ok(toggled_task)
    }

    pub async fn toggle_task_completion(
        &mut self,
        list_id: &str,
        task_id: &str,
        completed: bool,
    ) -> Result<TaskGet, reqwest::Error> {
        self.update_task(list_id, task_id, None, None, Some(completed), None)
            .await
    }

    async fn execute_with_retry(
        &mut self,
        build_request: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let request = build_request(&self.client, &self.access_token);
        let response = request.send().await?;

        //if error codde 401
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Ok(refresh_token) = crate::auth::keyring::get_refresh_token() {
                if let Ok(token_response) = crate::auth::refresh_access_token(&refresh_token).await
                {
                    self.access_token = token_response.access_token;

                    let retry_token = build_request(&self.client, &self.access_token);
                    return retry_token.send().await;
                }
            }
        }
        Ok(response)
    }

    pub async fn delete_task(
        &mut self,
        list_id: &str,
        task_id: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!(
            "https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}",
            list_id = list_id,
            task_id = task_id
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
            is_deleted: false,
        }
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
