use std::error::Error;
use std::time::Duration;

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::ui::{App, EditAction, InputMode};
use gtasks_core::{sync_local_to_db, sync_remote_to_db, Database, GoogleTasksClient, TaskList};

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
        let _ = crossterm::execute!(std::io::stdout(), Show);
    }
}

#[derive(Debug)]
pub enum BackgroundAction {
    TriggerFullSync,
    SaveTask(gtasks_core::api::TaskLocal),
    DeleteTask(String),
    SaveTaskList(TaskList),
}

/// Main TUI Terminal Controller & Event Loop
pub async fn run(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Load initial Task Lists and Tasks from SQLite cache
    let lists = db.get_task_lists()?;
    let first_list_id = lists.first().map(|l| l.id.as_str()).unwrap_or("");
    let tasks = if !first_list_id.is_empty() {
        db.get_tasks_for_list(first_list_id)?
    } else {
        Vec::new()
    };

    let mut app = App::new(lists, tasks);

    // 2. Terminal Setup (Enable Raw Mode & Alternate Screen)
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup background channel worker
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<BackgroundAction>();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<String>();

    let mut bg_client = client.clone();
    let mut bg_db = db.clone();

    tokio::spawn(async move {
        while let Some(action) = action_rx.recv().await {
            match action {
                BackgroundAction::TriggerFullSync => {
                    let _ = sync_local_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = sync_remote_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = result_tx.send("Synced with Google Tasks API! ✅".to_string());
                }
                BackgroundAction::SaveTask(task) => {
                    let _ = bg_db.save_tasks(std::slice::from_ref(&task));
                    let _ = sync_local_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = sync_remote_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = result_tx.send("Task saved & synced! ✅".to_string());
                }
                BackgroundAction::DeleteTask(task_id) => {
                    if task_id.starts_with("local_") {
                        let _ = bg_db.purge_task(&task_id);
                    } else {
                        let _ = bg_db.mark_task_deleted(&task_id);
                    }
                    let _ = sync_local_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = sync_remote_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = result_tx.send("Task deleted & synced! ✅".to_string());
                }
                BackgroundAction::SaveTaskList(list) => {
                    let _ = bg_db.save_task_lists(std::slice::from_ref(&list));
                    let _ = sync_local_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = sync_remote_to_db(&mut bg_client, &mut bg_db).await;
                    let _ = result_tx.send(format!("Saved Task List '{}' ✅", list.title));
                }
            }
        }
    });

    // Helper closure to reload tasks and lists from SQLite into memory
    let refresh_app_state = |app: &mut App, db: &Database| {
        if let Ok(updated_lists) = db.get_task_lists() {
            app.task_lists = updated_lists;
        }
        if let Some(selected_list) = app.selected_list() {
            if let Ok(updated_tasks) = db.get_tasks_for_list(&selected_list.id) {
                app.tasks = updated_tasks;
            }
        }
    };

    let update_task_pane_for_selected_list = |app: &mut App, db: &Database| {
        if app.active_pane == crate::ui::ActivePane::TaskLists {
            if let Some(selected_list) = app.selected_list() {
                if let Ok(list_tasks) = db.get_tasks_for_list(&selected_list.id) {
                    app.tasks = list_tasks;
                    app.selected_task_idx = 0;
                }
            }
        }
    };

    // 3. Event Loop
    loop {
        // Drain any results from background channel
        while let Ok(msg) = result_rx.try_recv() {
            refresh_app_state(&mut app, db);
            app.status_message = msg;
        }

        // Draw frame using Frontend renderer!
        terminal.draw(|f| crate::ui::draw::draw(f, &app))?;

        // Poll keypress events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Tab => {
                                app.switch_list();
                                update_task_pane_for_selected_list(&mut app, db);
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.select_up();
                                update_task_pane_for_selected_list(&mut app, db);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.select_down();
                                update_task_pane_for_selected_list(&mut app, db);
                            }
                            KeyCode::Char('g') => {
                                app.select_top();
                                update_task_pane_for_selected_list(&mut app, db);
                            }
                            KeyCode::Char('G') => {
                                app.select_bottom();
                                update_task_pane_for_selected_list(&mut app, db);
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_selected_task();
                                if let Some(task) = app.selected_task() {
                                    let _ = action_tx.send(BackgroundAction::SaveTask(task.clone()));
                                }
                            }
                            KeyCode::Char('c') => app.start_create_task(),
                            KeyCode::Char('L')
                                if app.active_pane == crate::ui::ActivePane::TaskLists =>
                            {
                                app.start_create_list();
                            }
                            KeyCode::Char('e') => app.start_edit_task(),
                            KeyCode::Char('d') | KeyCode::Delete => {
                                if let Some(removed) = app.delete_selected_task() {
                                    if !removed.id.is_empty() {
                                        let _ = action_tx.send(BackgroundAction::DeleteTask(removed.id.clone()));
                                    }
                                }
                            }
                            KeyCode::Char('r') => {
                                app.status_message = "Syncing with Google Tasks...".to_string();
                                let _ = action_tx.send(BackgroundAction::TriggerFullSync);
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Tab => {
                                app.cycle_edit_field();
                            }
                            KeyCode::Enter => {
                                if app.edit_action == EditAction::CreateList {
                                    let list_name = app.title_buffer.trim().to_string();
                                    if !list_name.is_empty() {
                                        let list_id = format!(
                                            "list_{}",
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis()
                                        );
                                        let new_list = TaskList {
                                            id: list_id,
                                            title: list_name.clone(),
                                            updated: Some(chrono::Utc::now().to_rfc3339()),
                                        };
                                        let _ = action_tx.send(BackgroundAction::SaveTaskList(new_list.clone()));
                                        app.task_lists.push(new_list);
                                        app.selected_list_idx = app.task_lists.len() - 1;
                                        app.tasks.clear();
                                        app.status_message =
                                            format!("Creating Task List '{}'...", list_name);
                                    }
                                    app.title_buffer.clear();
                                    app.input_mode = InputMode::Normal;
                                } else {
                                    app.submit_input();
                                    if let Some(task) = app.selected_task() {
                                        let _ = action_tx.send(BackgroundAction::SaveTask(task.clone()));
                                    }
                                    app.status_message = "Saving & syncing task...".to_string();
                                }
                            }
                            KeyCode::Esc => {
                                app.title_buffer.clear();
                                app.notes_buffer.clear();
                                app.due_buffer.clear();
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char(c) => {
                                app.active_buffer_mut().push(c);
                            }
                            KeyCode::Backspace => {
                                app.active_buffer_mut().pop();
                            }
                            _ => {}
                        },
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
