use std::fmt::format;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Local};

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
#[derive (Debug, Serialize, Deserialize, Clone)]
pub struct TaskGet {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>, // Task done or not 
    pub notes: Option<String>, // description of the task
    pub due: Option<String>, // Deadline 
    pub completed: Option<String>, // Completion date of the task
    pub parent: Option<String>, // Parent task ID (in case of subtask)
    pub updated: Option<String>, // Last modification date
}



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskLocal {
    pub id: String,
    pub list_id: String,
    pub title: Option<String>,
    pub is_completed: bool, // Task done or not 
    pub notes: Option<String>, // description of the task
    pub due: Option<chrono::DateTime<chrono::Utc>>, // Deadline 
    pub completed: Option<chrono::DateTime<chrono::Utc>>, // Completion date of the task
    pub parent: Option<String>, // Parent task ID (in case of subtask)
    pub updated: Option<chrono::DateTime<chrono::Utc>>, // Last modification date
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TasksResponse {
    pub items: Option<Vec<TaskGet>>,
}

// API
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
    pub async fn get_task_lists(&self) -> Result<Vec<TaskList>, reqwest::Error> {
        let url = "https://www.googleapis.com/tasks/v1/users/@me/lists";
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?;
        let response = response.error_for_status()?;

        let task_lists_response: TaskListsResponse = response.json().await?;
        Ok(task_lists_response.items.unwrap_or_default())
    }

    // Get the tasks for a specific task list
    pub async fn get_tasks (&self, list_id: &str, show_completed: bool) -> Result<Vec<TaskGet>, reqwest::Error> {
        let url = format!("https://www.googleapis.com/tasks/v1/lists/{}/tasks",list_id);

        let response = self
                                .client
                                .get(&url)
                                .bearer_auth(&self.access_token)
                                .query(&[("showCompleted", show_completed.to_string()), ("showHidden", show_completed.to_string())])
                                .send()
                                .await?;
        let response = response.error_for_status()?;
        let tasks_response: TasksResponse = response.json().await?;
        Ok(tasks_response.items.unwrap_or_default()) 
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
        }
    }
}