use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::ui::{ActivePane, App, EditAction, InputMode};

/// Renders the complete TUI Frontend visual layout with dynamic responsiveness & Multi-Field Detail Editor
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 1. Compute dynamic header height and formatted controls based on terminal width
    let (header_height, controls_text) = if area.width >= 115 {
        (
            3,
            " [Tab] Switch Pane | [Space] Toggle | [c] New Task | [L] New List | [e] Edit | [d] Delete | [r] Sync | [q] Quit",
        )
    } else if area.width >= 70 {
        (
            4,
            " [Tab] Switch Pane  |  [Space] Toggle Task  |  [c] New Task  |  [L] New List\n [e] Edit Details   |  [d] Delete Task   |  [r] Sync API  |  [q] Quit App",
        )
    } else {
        (
            5,
            " [Tab] Switch Pane | [Space] Toggle | [c] New Task\n [L] New List | [e] Edit | [d] Delete | [r] Sync\n [q] Quit App",
        )
    };

    // Cap header height on small screens (< 18 rows) to preserve task view area
    let final_header_height = if area.height < 18 && header_height > 3 {
        3
    } else {
        header_height
    };

    // Determine details panel height (5 rows on normal/large screens, 4 on small screens)
    let details_height = if area.height < 20 { 4 } else { 5 };

    // Vertical split: Top Controls Box vs Main Grid Body vs Bottom Task Details Panel
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(final_header_height),
            Constraint::Min(5),
            Constraint::Length(details_height),
        ])
        .split(area);

    // 2. Render Top Controls Rectangle Box
    let controls_header = Paragraph::new(controls_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" ⌨️ Controls & Status — {} ", app.status_message))
                .border_style(Style::default().fg(Color::LightMagenta)),
        )
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });
    frame.render_widget(controls_header, chunks[0]);

    // 3. Dynamic Horizontal split for Main Body: Left Sidebar vs Right Task Grid
    let sidebar_pct = if area.width < 80 { 40 } else { 30 };
    let task_pct = 100 - sidebar_pct;

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(sidebar_pct),
            Constraint::Percentage(task_pct),
        ])
        .split(chunks[1]);

    // 4. Render Task Lists Sidebar (Left)
    let list_items: Vec<ListItem> = app
        .task_lists
        .iter()
        .enumerate()
        .map(|(i, list)| {
            let style = if i == app.selected_list_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" 📁 {}", list.title)).style(style)
        })
        .collect();

    let list_border_style = if app.active_pane == ActivePane::TaskLists {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lists_widget = List::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Task Lists ")
            .border_style(list_border_style),
    );
    frame.render_widget(lists_widget, main_chunks[0]);

    // 5. Render Tasks Grid (Right)
    let task_items: Vec<ListItem> = app
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let icon = if task.is_completed { "✅" } else { "🔲" };
            let dirty = if task.is_dirty { " ⚡" } else { "" };
            let title = task.title.as_deref().unwrap_or("(No Title)");
            let content = format!(" {} {}{}", icon, title, dirty);

            let style = if i == app.selected_task_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let task_border_style = if app.active_pane == ActivePane::Tasks {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tasks_widget = List::new(task_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Tasks ")
            .border_style(task_border_style),
    );
    frame.render_widget(tasks_widget, main_chunks[1]);

    // 6. Render Dedicated Task Details Panel (Bottom)
    let details_content = if let Some(selected) = app.selected_task() {
        let title = selected.title.as_deref().unwrap_or("(No Title)");
        let notes = selected.notes.as_deref().unwrap_or("(No Description)");
        let due_str = selected
            .due
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "None".to_string());
        let status_str = if selected.is_completed {
            "Completed ✅"
        } else {
            "Pending 🔲"
        };
        let dirty_str = if selected.is_dirty { " | Unsynced ⚡" } else { "" };

        format!(
            " 📌 Title: {}\n 📝 Description: {}\n 📅 Due: {} | Status: {}{}",
            title, notes, due_str, status_str, dirty_str
        )
    } else {
        " No task selected.".to_string()
    };

    let details_panel = Paragraph::new(details_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📌 Task Details ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(details_panel, chunks[2]);

    // 7. Render Centered Input Popup Modal when Editing / Creating
    if app.input_mode == InputMode::Editing {
        match app.edit_action {
            EditAction::CreateTask => {
                let area = centered_rect(60, 25, frame.area());
                frame.render_widget(Clear, area);
                let input_widget = Paragraph::new(app.title_buffer.as_str())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" 📝 Create New Task (Title) ")
                            .border_style(Style::default().fg(Color::Yellow)),
                    )
                    .wrap(Wrap { trim: true });
                frame.render_widget(input_widget, area);
            }
            EditAction::CreateList => {
                let area = centered_rect(60, 25, frame.area());
                frame.render_widget(Clear, area);
                let input_widget = Paragraph::new(app.title_buffer.as_str())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" 📁 Create New Task List (Name) ")
                            .border_style(Style::default().fg(Color::Yellow)),
                    )
                    .wrap(Wrap { trim: true });
                frame.render_widget(input_widget, area);
            }
            EditAction::EditTaskDetails => {
                let area = centered_rect(65, 50, frame.area());
                frame.render_widget(Clear, area);

                let outer_block = Block::default()
                    .borders(Borders::ALL)
                    .title(" 📝 Edit Task Details — Press [Tab] to switch fields, [Enter] to save ")
                    .border_style(Style::default().fg(Color::Yellow));
                frame.render_widget(outer_block, area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Title
                        Constraint::Min(3),    // Description / Notes
                        Constraint::Length(3), // Due Date
                    ])
                    .margin(1)
                    .split(area);

                let title_style = if app.edit_field == crate::ui::EditField::Title {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let notes_style = if app.edit_field == crate::ui::EditField::Notes {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let due_style = if app.edit_field == crate::ui::EditField::Due {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let title_w = Paragraph::new(app.title_buffer.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Title ")
                        .border_style(title_style),
                );
                let notes_w = Paragraph::new(app.notes_buffer.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Description / Notes ")
                        .border_style(notes_style),
                );
                let due_w = Paragraph::new(app.due_buffer.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Due Date (YYYY-MM-DD) ")
                        .border_style(due_style),
                );

                frame.render_widget(title_w, field_chunks[0]);
                frame.render_widget(notes_w, field_chunks[1]);
                frame.render_widget(due_w, field_chunks[2]);
            }
        }
    }
}

/// Helper function to calculate a centered popup rectangle for modals
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
