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
}

