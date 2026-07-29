use chrono::Datelike;
use gtasks_core::{TaskList, TaskLocal};

pub mod draw;
pub mod run;
pub use run::run;

#[derive(Debug, PartialEq, Eq)]
pub enum ActivePane {
    TaskLists,
    Tasks,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditAction {
    CreateTask,
    CreateList,
    EditTaskDetails,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EditField {
    Title,
    Notes,
    Due,
}

pub struct App {
    pub task_lists: Vec<TaskList>,
    pub tasks: Vec<TaskLocal>,
    pub selected_list_idx: usize,
    pub selected_task_idx: usize,
    pub active_pane: ActivePane,
    pub should_quit: bool,
    pub status_message: String,

    pub input_mode: InputMode,
    pub edit_action: EditAction,
    pub edit_field: EditField,
    pub title_buffer: String,
    pub notes_buffer: String,
    pub due_buffer: String,
}

pub fn parse_nlp_task(input: &str) -> (String, Option<chrono::DateTime<chrono::Utc>>) {
    let lower = input.to_lowercase();
    let today = chrono::Local::now().date_naive();
    let mut due_date: Option<chrono::NaiveDate> = None;
    let mut matched_phrase: Option<&str> = None;

    if lower.contains("next week") {
        due_date = Some(today + chrono::Duration::days(7));
        matched_phrase = Some("next week");
    } else if lower.contains("tomorrow") {
        due_date = Some(today + chrono::Duration::days(1));
        matched_phrase = Some("tomorrow");
    } else if lower.contains("today") {
        due_date = Some(today);
        matched_phrase = Some("today");
    } else {
        let patterns = [
            ("next monday", chrono::Weekday::Mon),
            ("next tuesday", chrono::Weekday::Tue),
            ("next wednesday", chrono::Weekday::Wed),
            ("next thursday", chrono::Weekday::Thu),
            ("next friday", chrono::Weekday::Fri),
            ("next saturday", chrono::Weekday::Sat),
            ("next sunday", chrono::Weekday::Sun),
        ];
        for (phrase, weekday) in patterns {
            if lower.contains(phrase) {
                let mut d = today + chrono::Duration::days(1);
                while d.weekday() != weekday {
                    d += chrono::Duration::days(1);
                }
                due_date = Some(d);
                matched_phrase = Some(phrase);
                break;
            }
        }

        if due_date.is_none() {
            let single_weekdays = [
                ("monday", chrono::Weekday::Mon),
                ("tuesday", chrono::Weekday::Tue),
                ("wednesday", chrono::Weekday::Wed),
                ("thursday", chrono::Weekday::Thu),
                ("friday", chrono::Weekday::Fri),
                ("saturday", chrono::Weekday::Sat),
                ("sunday", chrono::Weekday::Sun),
            ];
            for (name, weekday) in single_weekdays {
                let words: Vec<&str> = lower.split_whitespace().collect();
                if words.contains(&name) {
                    let mut d = today + chrono::Duration::days(1);
                    while d.weekday() != weekday {
                        d += chrono::Duration::days(1);
                    }
                    due_date = Some(d);
                    matched_phrase = Some(name);
                    break;
                }
            }
        }
    }

    let clean_title = if let Some(phrase) = matched_phrase {
        let mut result = String::new();
        let mut remaining = input;
        while let Some(pos) = remaining.to_lowercase().find(phrase) {
            result.push_str(&remaining[..pos]);
            remaining = &remaining[pos + phrase.len()..];
        }
        result.push_str(remaining);
        result
    } else {
        input.to_string()
    };

    let clean_trimmed = clean_title.trim().to_string();
    let final_title = if clean_trimmed.is_empty() {
        input.trim().to_string()
    } else {
        clean_trimmed
    };

    let due_dt = due_date.map(|d| {
        let naive_dt = d.and_hms_opt(0, 0, 0).unwrap();
        chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive_dt, chrono::Utc)
    });

    (final_title, due_dt)
}

impl App {
    pub fn new(task_lists: Vec<TaskList>, tasks: Vec<TaskLocal>) -> Self {
        Self {
            task_lists,
            tasks,
            selected_list_idx: 0,
            selected_task_idx: 0,
            active_pane: ActivePane::TaskLists,
            should_quit: false,
            status_message:
                "Ready. Press 'q' to quit, 'Tab' to switch panels, 'Space' to toggle task."
                    .to_string(),
            input_mode: InputMode::Normal,
            edit_action: EditAction::CreateTask,
            edit_field: EditField::Title,
            title_buffer: String::new(),
            notes_buffer: String::new(),
            due_buffer: String::new(),
        }
    }

    pub fn active_buffer_mut(&mut self) -> &mut String {
        match self.edit_field {
            EditField::Title => &mut self.title_buffer,
            EditField::Notes => &mut self.notes_buffer,
            EditField::Due => &mut self.due_buffer,
        }
    }

    pub fn cycle_edit_field(&mut self) {
        if self.edit_action == EditAction::EditTaskDetails {
            self.edit_field = match self.edit_field {
                EditField::Title => EditField::Notes,
                EditField::Notes => EditField::Due,
                EditField::Due => EditField::Title,
            };
        }
    }

    // Navigate up
    pub fn select_up(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if self.selected_list_idx > 0 {
                    self.selected_list_idx -= 1;
                }
            }
            ActivePane::Tasks => {
                if self.selected_task_idx > 0 {
                    self.selected_task_idx -= 1;
                }
            }
        }
    }

    // Navigate down
    pub fn select_down(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if !self.task_lists.is_empty()
                    && self.selected_list_idx < self.task_lists.len() - 1
                {
                    self.selected_list_idx += 1;
                }
            }
            ActivePane::Tasks => {
                if !self.tasks.is_empty() && self.selected_task_idx < self.tasks.len() - 1 {
                    self.selected_task_idx += 1;
                }
            }
        }
    }

    // Navigate to top
    pub fn select_top(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => self.selected_list_idx = 0,
            ActivePane::Tasks => self.selected_task_idx = 0,
        }
    }

    // Navigate to bottom
    pub fn select_bottom(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if !self.task_lists.is_empty() {
                    self.selected_list_idx = self.task_lists.len() - 1;
                }
            }
            ActivePane::Tasks => {
                if !self.tasks.is_empty() {
                    self.selected_task_idx = self.tasks.len() - 1;
                }
            }
        }
    }

    // Switch focus between list of task_list and tasks
    pub fn switch_list(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::TaskLists => ActivePane::Tasks,
            ActivePane::Tasks => ActivePane::TaskLists,
        };
    }

    pub fn selected_list(&self) -> Option<&TaskList> {
        self.task_lists.get(self.selected_list_idx)
    }

    pub fn selected_task(&self) -> Option<&TaskLocal> {
        self.tasks.get(self.selected_task_idx)
    }

    pub fn selected_task_mut(&mut self) -> Option<&mut TaskLocal> {
        self.tasks.get_mut(self.selected_task_idx)
    }

    pub fn toggle_selected_task(&mut self) {
        if self.active_pane == ActivePane::Tasks {
            if let Some(task) = self.selected_task_mut() {
                task.is_completed = !task.is_completed;
                task.is_dirty = true;

                if task.is_completed {
                    task.completed = Some(chrono::Utc::now());
                    self.status_message = format!(
                        "Marked task '{}' as completed ✅",
                        task.title.as_deref().unwrap_or("")
                    );
                } else {
                    task.completed = None;
                    self.status_message = format!(
                        "Marked task '{}' as pending 🔲",
                        task.title.as_deref().unwrap_or("")
                    );
                }
            }
        }
    }

    pub fn delete_selected_task(&mut self) -> Option<TaskLocal> {
        if self.active_pane == ActivePane::Tasks && !self.tasks.is_empty() {
            let removed = self.tasks.remove(self.selected_task_idx);

            if self.selected_task_idx >= self.tasks.len() && self.selected_task_idx > 0 {
                self.selected_task_idx -= 1;
            }
            self.status_message =
                format!("Deleted task '{}'", removed.title.as_deref().unwrap_or(""));
            return Some(removed);
        }
        None
    }

    pub fn start_create_task(&mut self) {
        self.input_mode = InputMode::Editing;
        self.edit_action = EditAction::CreateTask;
        self.edit_field = EditField::Title;
        self.title_buffer.clear();
        self.notes_buffer.clear();
        self.due_buffer.clear();
        self.status_message =
            "Type task title and press Enter to save (Esc to cancel)...".to_string();
    }

    pub fn start_create_list(&mut self) {
        self.input_mode = InputMode::Editing;
        self.edit_action = EditAction::CreateList;
        self.edit_field = EditField::Title;
        self.title_buffer.clear();
        self.status_message =
            "Type new Task List name and press Enter to save (Esc to cancel)...".to_string();
    }

    pub fn start_edit_task(&mut self) {
        if self.active_pane == ActivePane::Tasks {
            let task_info = self.selected_task().map(|t| {
                (
                    t.title.clone().unwrap_or_default(),
                    t.notes.clone().unwrap_or_default(),
                    t.due
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default(),
                )
            });

            if let Some((title, notes, due)) = task_info {
                self.title_buffer = title;
                self.notes_buffer = notes;
                self.due_buffer = due;

                self.input_mode = InputMode::Editing;
                self.edit_action = EditAction::EditTaskDetails;
                self.edit_field = EditField::Title;
                self.status_message =
                    "Editing task details. Use [Tab] to switch fields, [Enter] to save..."
                        .to_string();
            }
        }
    }

    pub fn submit_input(&mut self) {
        match self.edit_action {
            EditAction::CreateTask => {
                let (clean_title, due_dt) = parse_nlp_task(&self.title_buffer);
                if !clean_title.is_empty() {
                    if let Some(current_list) = self.selected_list() {
                        let task_id = format!(
                            "local_{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                        );

                        let new_task = TaskLocal {
                            id: task_id,
                            list_id: current_list.id.clone(),
                            title: Some(clean_title.clone()),
                            is_completed: false,
                            notes: None,
                            due: due_dt,
                            completed: None,
                            parent: None,
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                            is_deleted: false,
                        };
                        self.tasks.push(new_task);
                        self.selected_task_idx = self.tasks.len() - 1;
                        self.status_message = format!("Created new task '{}'", clean_title);
                    }
                }
            }
            EditAction::CreateList => {
                // Handled in run loop to invoke API client or main DB
            }
            EditAction::EditTaskDetails => {
                let new_title = self.title_buffer.trim().to_string();
                let new_notes = if self.notes_buffer.trim().is_empty() {
                    None
                } else {
                    Some(self.notes_buffer.trim().to_string())
                };

                let new_due = if self.due_buffer.trim().is_empty() {
                    None
                } else {
                    chrono::NaiveDate::parse_from_str(self.due_buffer.trim(), "%Y-%m-%d")
                        .ok()
                        .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
                        .map(|dt| {
                            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                                dt,
                                chrono::Utc,
                            )
                        })
                };

                if let Some(task) = self.selected_task_mut() {
                    task.title = Some(new_title.clone());
                    task.notes = new_notes;
                    task.due = new_due;
                    task.updated = Some(chrono::Utc::now());
                    task.is_dirty = true;
                    self.status_message = format!("Updated task '{}'", new_title);
                }
            }
        }

        self.title_buffer.clear();
        self.notes_buffer.clear();
        self.due_buffer.clear();
        self.input_mode = InputMode::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nlp_task_today_tomorrow() {
        let (title1, due1) = parse_nlp_task("Buy milk today");
        assert_eq!(title1, "Buy milk");
        assert!(due1.is_some());

        let (title2, due2) = parse_nlp_task("Finish report tomorrow");
        assert_eq!(title2, "Finish report");
        assert!(due2.is_some());
    }

    #[test]
    fn test_parse_nlp_task_no_date() {
        let (title, due) = parse_nlp_task("Read documentation");
        assert_eq!(title, "Read documentation");
        assert!(due.is_none());
    }

    #[test]
    fn test_select_top_and_bottom() {
        let mut app = App::new(vec![], vec![]);
        app.task_lists = vec![
            TaskList { id: "1".into(), title: "L1".into(), updated: None },
            TaskList { id: "2".into(), title: "L2".into(), updated: None },
            TaskList { id: "3".into(), title: "L3".into(), updated: None },
        ];
        app.select_bottom();
        assert_eq!(app.selected_list_idx, 2);
        app.select_top();
        assert_eq!(app.selected_list_idx, 0);
    }
}
