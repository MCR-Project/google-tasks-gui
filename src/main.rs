mod api;
mod auth;
mod db;

use api::GoogleTasksClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let token_response = if let Ok(refresh_token) = auth::keyring::get_refresh_token() {
        println!("Found refresh token in keyring. Refreshing access token...");
        auth::refresh_access_token(&refresh_token).await?
    } else {
        println!("No refresh token found in keyring. Starting authentication process...");
        let token_response = auth::authenticate().await?;
        if let Some(ref refresh_token) = token_response.refresh_token {
            auth::keyring::save_refresh_token(refresh_token)?;
            println!("Refresh token saved to keyring.");
        }
        token_response
    };

    // 1. Authenticate and get the access token
    println!(
        "✅ Authentication successful! Access Token: {}",
        token_response.access_token
    );

    if let Some(ref refresh_token) = token_response.refresh_token {
        println!("✅ Refresh Token: {}", refresh_token);
    }

    //initialize db
    let mut db = db::Database::new("task_lists.db")?;

    // initialize api client with access token
    // Instantiate the client from our api module
    let client = GoogleTasksClient::new(token_response.access_token);

    println!("Fetching Task Lists from Google Tasks API...");

    // Fetch task lists from Google Tasks API
    match client.get_task_lists().await {
        Ok(lists) => {
            println!("\n📋 Retrieved {} Task List(s):", lists.len());
            db.save_task_lists(&lists)?;

            for list in &lists {
                let list_id = &list.id;
                let new_task = api::TaskLocal {
                    id: String::new(),
                    list_id: list_id.to_string(),
                    title: Some("New Task Title".to_string()),
                    is_completed: false,
                    notes: Some("This is a new task.".to_string()),
                    due: None,
                    completed: None,
                    parent: None,
                    updated: None,
                };
                // 1. Notice Ok(raw_tasks) without type annotation
                match client.get_tasks(list_id, true).await {
                    Ok(raw_tasks) => {
                        println!(
                            "\n📝 Retrieved {} Task(s) for List ID '{}':",
                            raw_tasks.len(),
                            list_id
                        );

                        let mut local_tasks: Vec<api::TaskLocal> = Vec::new();

                        // 2. Loop variable is named raw_task
                        for raw_task in raw_tasks {
                            // Convert raw TaskGet into clean TaskLocal
                            let task = api::TaskLocal::from_task_get(raw_task, list.id.clone());

                            let icon = if task.is_completed { "✅" } else { "🔲" };
                            let title = task.title.as_deref().unwrap_or("(No Title)");

                            println!("   {} [{}] {}", icon, task.id, title);

                            local_tasks.push(task);
                        }

                        db.save_tasks(&local_tasks)?;
                        println!(
                            "💾 Saved {} Task(s) for List ID '{}' to the database.",
                            local_tasks.len(),
                            list_id
                        );
                    }
                    Err(err) => eprintln!("❌ Error fetching tasks for list {}: {}", list_id, err),
                }

                match client.create_task(&list_id, &new_task).await {
                    Ok(created_raw_task) => {
                        let created_task =
                            api::TaskLocal::from_task_get(created_raw_task, list_id.to_string());
                        println!(
                            "\n✅ Successfully created task with ID: {} and Title: {}",
                            created_task.id,
                            created_task.title.as_deref().unwrap_or("(No Title)")
                        );
                        db.save_tasks(&[created_task])?;
                    }
                    Err(err) => eprintln!("❌ Error creating task: {}", err),
                }
            }
        }
        Err(err) => eprintln!("❌ Error fetching task lists: {}", err),
    }

    // Store in db
    let stored_task_lists = db.get_task_lists()?;
    println!("\n💾 Stored Task Lists in Database:");
    for list in stored_task_lists {
        println!("  • [{}] {}", list.id, list.title);
    }

    Ok(())
}
