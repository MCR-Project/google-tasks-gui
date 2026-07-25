mod auth;
mod db;
mod api;

use api::{GoogleTasksClient};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    // 1. Authenticate and get the access token
    println!("Starting authentication process...");
    let token_response = auth::authenticate().await?;
    println!("✅ Authentication successful! Access Token: {}", token_response.access_token);

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
            for list in &lists {
                println!("  • [{}] {}", list.id, list.title);
                let status_icon = if list.updated.is_some() { "✅" } else { "⚠️" };
                println!("    Updated: {} {}", status_icon, list.updated.as_deref().unwrap_or("N/A"));
                
            }
            // Save the fetched task lists to the database
            db.save_task_lists(&lists)?;
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