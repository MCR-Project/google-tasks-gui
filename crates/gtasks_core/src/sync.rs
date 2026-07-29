use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

use crate::db::Database;
use crate::{obtain_authenticated_client, sync_local_to_db, sync_remote_to_db_delta};

#[derive(Debug, Clone)]
pub enum SyncCommand {
    TriggerImmediateSync,
    SetWindowActive(bool),
}

#[derive(Debug, Clone)]
pub enum SyncEvent {
    SyncStarted { is_manual: bool },
    SyncFinished(Result<DateTime<Utc>, String>),
}

pub struct SyncManager {
    cmd_tx: mpsc::Sender<SyncCommand>,
}

impl SyncManager {
    pub fn spawn(event_tx: mpsc::Sender<SyncEvent>) -> Arc<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);

        tokio::spawn(async move {
            run_sync_actor(cmd_rx, event_tx).await;
        });

        Arc::new(Self { cmd_tx })
    }

    pub fn trigger_sync(&self) {
        let _ = self.cmd_tx.try_send(SyncCommand::TriggerImmediateSync);
    }

    pub fn set_window_active(&self, active: bool) {
        let _ = self.cmd_tx.try_send(SyncCommand::SetWindowActive(active));
    }
}

async fn run_sync_actor(
    mut cmd_rx: mpsc::Receiver<SyncCommand>,
    event_tx: mpsc::Sender<SyncEvent>,
) {
    let mut is_active = true;
    let mut last_sync_time: Option<DateTime<Utc>> = None;

    let active_duration = Duration::from_secs(30);
    let idle_duration = Duration::from_secs(180);

    let mut timer = time::interval(active_duration);
    timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SyncCommand::TriggerImmediateSync) => {
                        let _ = event_tx.send(SyncEvent::SyncStarted { is_manual: true }).await;
                        let res = perform_single_sync(&mut last_sync_time).await;
                        let _ = event_tx.send(SyncEvent::SyncFinished(res)).await;
                        timer.reset();
                    }
                    Some(SyncCommand::SetWindowActive(active)) => {
                        let became_active = active && !is_active;
                        is_active = active;
                        let new_duration = if is_active { active_duration } else { idle_duration };
                        timer = time::interval(new_duration);
                        timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

                        if became_active {
                            println!("🪟 Window focused! Triggering immediate background delta sync...");
                            let _ = event_tx.send(SyncEvent::SyncStarted { is_manual: false }).await;
                            let res = perform_single_sync(&mut last_sync_time).await;
                            let _ = event_tx.send(SyncEvent::SyncFinished(res)).await;
                        }
                    }
                    None => break,
                }
            }
            _ = timer.tick() => {
                let _ = event_tx.send(SyncEvent::SyncStarted { is_manual: false }).await;
                let res = perform_single_sync(&mut last_sync_time).await;
                let _ = event_tx.send(SyncEvent::SyncFinished(res)).await;
            }
        }
    }
}

async fn perform_single_sync(
    last_sync_time: &mut Option<DateTime<Utc>>,
) -> Result<DateTime<Utc>, String> {
    let sync_start = Utc::now();
    let mut client = obtain_authenticated_client()
        .await
        .map_err(|e| e.to_string())?;
    let mut db = Database::new("task_lists.db").map_err(|e| e.to_string())?;

    sync_local_to_db(&mut client, &mut db)
        .await
        .map_err(|e| e.to_string())?;

    sync_remote_to_db_delta(&mut client, &mut db, *last_sync_time)
        .await
        .map_err(|e| e.to_string())?;

    *last_sync_time = Some(sync_start);
    Ok(sync_start)
}
