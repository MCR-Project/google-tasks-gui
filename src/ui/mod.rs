use crate::api::{TaskList, TaskLocal};

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
                if !self.task_lists.is_empty() && self.selected_list_idx < self.task_lists.len() - 1
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
            let task_info = self.selected_task().map(|t| (
                t.title.clone().unwrap_or_default(),
                t.notes.clone().unwrap_or_default(),
                t.due.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            ));

            if let Some((title, notes, due)) = task_info {
                self.title_buffer = title;
                self.notes_buffer = notes;
                self.due_buffer = due;

                self.input_mode = InputMode::Editing;
                self.edit_action = EditAction::EditTaskDetails;
                self.edit_field = EditField::Title;
                self.status_message =
                    "Editing task details. Use [Tab] to switch fields, [Enter] to save...".to_string();
            }
        }
    }

    pub fn submit_input(&mut self) {
        match self.edit_action {
            EditAction::CreateTask => {
                let title = self.title_buffer.trim().to_string();
                if !title.is_empty() {
                    if let Some(current_list) = self.selected_list() {
                        let new_task = TaskLocal {
                            id: String::new(),
                            list_id: current_list.id.clone(),
                            title: Some(title.clone()),
                            is_completed: false,
                            notes: None,
                            due: None,
                            completed: None,
                            parent: None,
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                        };
                        self.tasks.push(new_task);
                        self.selected_task_idx = self.tasks.len() - 1;
                        self.status_message = format!("Created new task '{}'", title);
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
                        .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
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
