mod ui;

use gtasks_core::{
    obtain_authenticated_client, sync_local_to_db, sync_remote_to_db, Database, GoogleTasksClient,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    println!("🚀 Starting gTasks Terminal TUI...\n");

    let rt = tokio::runtime::Runtime::new()?;

    // Step 1 & 2: Authenticate and initialize SQLite DB
    let (mut client, mut db) = rt.block_on(async {
        let client = obtain_authenticated_client().await?;
        let db = Database::new("task_lists.db")?;
        Ok::<(GoogleTasksClient, Database), Box<dyn Error + Send + Sync>>((client, db))
    })?;

    // Step 3: Synchronize remote & local data on startup
    println!("🔄 Syncing data with Google Tasks API...");
    rt.block_on(async {
        let _ = sync_remote_to_db(&mut client, &mut db).await;
        let _ = sync_local_to_db(&mut client, &mut db).await;
    });

    // Step 4: Run TUI Interface
    rt.block_on(async {
        ui::run(&mut client, &mut db).await
    })?;

    println!("\n✨ Thank you for using gTasks!");
    Ok(())
}
