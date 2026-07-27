use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::ui::{ActivePane, App, EditAction, InputMode};

/// Renders the complete TUI Frontend visual layout
pub fn draw(frame: &mut Frame, app: &App) {
    // 1. Vertical split: Main Body vs Bottom Status Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(frame.area());

    // 2. Horizontal split: Left Sidebar (30%) vs Right Task List Grid (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[0]);

    // 3. Render Task Lists Sidebar (Left)
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

    // 4. Render Tasks Grid (Right)
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

    // 5. Render Bottom Status Bar (Footer) with text wrapping
    let footer = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::White).bg(Color::Blue))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[1]);

    // 6. Render Centered Input Popup Modal when Editing / Creating
    if app.input_mode == InputMode::Editing {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area); // Clear underlying UI behind popup

        let popup_title = match app.edit_action {
            EditAction::Create => " Create New Task ",
            EditAction::EditTitle => " Edit Task Title ",
        };

        let input_widget = Paragraph::new(app.input_buffer.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(popup_title)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(input_widget, area);
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
