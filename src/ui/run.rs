use std::error::Error;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::api::GoogleTasksClient;
use crate::db::Database;
use crate::ui::{App, InputMode};

/// Main TUI Terminal Controller & Event Loop
pub async fn run(
    client: &mut GoogleTasksClient,
    db: &mut Database,
) -> Result<(), Box<dyn Error>> {
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
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Event Loop
    loop {
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
                                // Refresh displayed tasks when switching list selection
                                if let Some(selected_list) = app.selected_list() {
                                    if let Ok(list_tasks) = db.get_tasks_for_list(&selected_list.id) {
                                        app.tasks = list_tasks;
                                        app.selected_task_idx = 0;
                                    }
                                }
                            }
                            KeyCode::Up => {
                                app.select_up();
                                if let Some(selected_list) = app.selected_list() {
                                    if let Ok(list_tasks) = db.get_tasks_for_list(&selected_list.id) {
                                        app.tasks = list_tasks;
                                    }
                                }
                            }
                            KeyCode::Down => {
                                app.select_down();
                                if let Some(selected_list) = app.selected_list() {
                                    if let Ok(list_tasks) = db.get_tasks_for_list(&selected_list.id) {
                                        app.tasks = list_tasks;
                                    }
                                }
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_selected_task();
                                if let Some(task) = app.selected_task() {
                                    let _ = db.save_tasks(&[task.clone()]);
                                }
                            }
                            KeyCode::Char('c') => app.start_create_task(),
                            KeyCode::Char('e') => app.start_edit_task(),
                            KeyCode::Char('d') | KeyCode::Delete => {
                                if let Some(removed) = app.delete_selected_task() {
                                    if !removed.id.is_empty() {
                                        let _ = db.delete_tasks_db(&[removed.id]);
                                    }
                                }
                            }
                            KeyCode::Char('r') => {
                                app.status_message = "Syncing with Google Tasks...".to_string();
                                terminal.draw(|f| crate::ui::draw::draw(f, &app))?;
                                let _ = crate::sync_local_to_db(client, db).await;
                                let _ = crate::sync_remote_to_db(client, db).await;
                                app.status_message = "Synced with Google Tasks API! ✅".to_string();
                            }
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => {
                                app.submit_input();
                                if let Some(task) = app.selected_task() {
                                    let _ = db.save_tasks(&[task.clone()]);
                                }
                            }
                            KeyCode::Esc => app.input_mode = InputMode::Normal,
                            KeyCode::Char(c) => app.input_buffer.push(c),
                            KeyCode::Backspace => {
                                app.input_buffer.pop();
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

    // 4. Terminal Teardown (Restore original terminal screen)
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
