use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::ui::{ActivePane, App, EditAction, EditField, InputMode};

/// Renders the complete TUI Frontend visual layout with dynamic responsiveness & Inline Task Inspector Pane
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 1. Compute dynamic header height and formatted controls based on terminal width
    let (header_height, controls_body) = if area.width >= 120 {
        (
            4,
            format!(
                " Status: {}\n Controls: [Tab] Switch Pane | [j/k/g/G/↑/↓] Navigate | [Space] Toggle | [c] New Task | [L] New List | [e] Edit | [d] Delete | [r] Sync | [q] Quit",
                app.status_message
            ),
        )
    } else if area.width >= 75 {
        (
            5,
            format!(
                " Status: {}\n Controls: [Tab] Switch Pane | [j/k/g/G] Nav | [Space] Toggle | [c] New Task | [L] New List\n           [e] Edit Inspector | [d] Delete | [r] Sync API | [q] Quit",
                app.status_message
            ),
        )
    } else {
        (
            6,
            format!(
                " Status: {}\n Controls: [Tab] Switch | [j/k/g/G] Nav | [Space] Toggle\n           [c] New Task | [L] New List | [e] Edit\n           [d] Delete | [r] Sync | [q] Quit",
                app.status_message
            ),
        )
    };

    let final_header_height = if area.height < 22 && header_height > 4 {
        4
    } else {
        header_height
    };

    // Vertical split: Top Controls Box vs Main 3-Pane Body
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(final_header_height),
            Constraint::Min(5),
        ])
        .split(area);

    // 2. Render Top Controls Rectangle Box (Responsive multi-line layout)
    let controls_header = Paragraph::new(controls_body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⌨️ Controls & Status ")
                .border_style(Style::default().fg(Color::LightMagenta)),
        )
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });
    frame.render_widget(controls_header, chunks[0]);

    // 3. Dynamic Horizontal split for Main Body: Left Sidebar (Lists) vs Right Main Split (Task List 60% / Inspector 40%)
    let sidebar_width = if area.width < 100 { 20 } else { 24 };

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(sidebar_width),
            Constraint::Min(10),
        ])
        .split(chunks[1]);

    // 4. Render Task Lists Sidebar (Left)
    let is_creating_list = app.input_mode == InputMode::Editing
        && app.edit_action == EditAction::CreateList;

    let mut list_items: Vec<ListItem> = Vec::new();

    if is_creating_list {
        let input_text = format!(" ✍️ {}{} ", app.title_buffer, "█");
        let active_style = Style::default()
            .fg(Color::Yellow)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        list_items.push(ListItem::new(input_text).style(active_style));
    }

    for (i, list) in app.task_lists.iter().enumerate() {
        let style = if i == app.selected_list_idx && !is_creating_list {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        list_items.push(ListItem::new(format!(" 📁 {}", list.title)).style(style));
    }

    let list_border_style = if is_creating_list {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if app.active_pane == ActivePane::TaskLists {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list_title = if is_creating_list {
        format!(" Task Lists — ✍️ New: {}{} ", app.title_buffer, "█")
    } else {
        " Task Lists ".to_string()
    };

    let lists_widget = List::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(list_title)
            .border_style(list_border_style),
    );
    frame.render_widget(lists_widget, main_chunks[0]);

    // 5. Split remaining area into Task List (60%) and Inline Task Inspector Pane (40%)
    let task_area_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(main_chunks[1]);

    // 6. Render Tasks List Pane (Center / 60%)
    let is_creating_task = app.input_mode == InputMode::Editing
        && app.edit_action == EditAction::CreateTask;

    let mut task_items: Vec<ListItem> = Vec::new();

    if is_creating_task {
        let input_text = format!(" ✍️ {}{} ", app.title_buffer, "█");
        let active_style = Style::default()
            .fg(Color::Yellow)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        task_items.push(ListItem::new(input_text).style(active_style));
    }

    for (i, task) in app.tasks.iter().enumerate() {
        let icon = if task.is_completed { "✅" } else { "🔲" };
        let dirty = if task.is_dirty { " ⚡" } else { "" };
        let title = task.title.as_deref().unwrap_or("(No Title)");
        let prefix = if task.parent.is_some() { "  ↳ " } else { " " };
        let content = format!("{}{}{}{}", prefix, icon, title, dirty);

        let style = if i == app.selected_task_idx && !is_creating_task {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        task_items.push(ListItem::new(content).style(style));
    }

    let task_border_style = if is_creating_task {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if app.active_pane == ActivePane::Tasks && app.input_mode == InputMode::Normal {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tasks_title = if is_creating_task {
        format!(" Tasks — ✍️ Creating: {}{} ", app.title_buffer, "█")
    } else {
        " Tasks ".to_string()
    };

    let tasks_widget = List::new(task_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(tasks_title)
            .border_style(task_border_style),
    );
    frame.render_widget(tasks_widget, task_area_chunks[0]);

    // 7. Render Inline Task Inspector Pane (Right / 40%)
    let inspector_border_style = if app.input_mode == InputMode::Editing
        && app.edit_action == EditAction::EditTaskDetails
    {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if app.input_mode == InputMode::Editing
        && app.edit_action == EditAction::EditTaskDetails
    {
        // Interactive Inline Editor Mode in Inspector Pane
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .title(" 📝 Edit Inspector ([Tab] Switch field) ")
            .border_style(inspector_border_style);
        frame.render_widget(outer_block, task_area_chunks[1]);

        let field_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(3),    // Description / Notes
                Constraint::Length(3), // Due Date
            ])
            .margin(1)
            .split(task_area_chunks[1]);

        let title_style = if app.edit_field == EditField::Title {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let notes_style = if app.edit_field == EditField::Notes {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let due_style = if app.edit_field == EditField::Due {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
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
                .title(" Notes ")
                .border_style(notes_style),
        );
        let due_w = Paragraph::new(app.due_buffer.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Due (YYYY-MM-DD) ")
                .border_style(due_style),
        );

        frame.render_widget(title_w, field_chunks[0]);
        frame.render_widget(notes_w, field_chunks[1]);
        frame.render_widget(due_w, field_chunks[2]);
    } else {
        // Read-only Live Inspection Mode in Inspector Pane
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
            let dirty_str = if selected.is_dirty {
                "\n ⚡ Unsynced Local Edit"
            } else {
                ""
            };

            format!(
                " 📌 Title:\n  {}\n\n 📝 Notes:\n  {}\n\n 📅 Due:\n  {}\n\n ⚙️ Status:\n  {}{}",
                title, notes, due_str, status_str, dirty_str
            )
        } else {
            "\n No task selected.".to_string()
        };

        let details_panel = Paragraph::new(details_content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 📌 Task Inspector ")
                    .border_style(inspector_border_style),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true });
        frame.render_widget(details_panel, task_area_chunks[1]);
    }
}
