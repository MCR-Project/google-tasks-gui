use crate::{
    api::{TaskList, TaskLocal},
    ui::ActivePane::TaskLists,
};

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

#[derive(Debug)]
pub enum EditAction {
    Create,
    EditTitle,
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
    pub input_buffer: String,
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
            edit_action: EditAction::Create,
            input_buffer: String::new(),
        }
    }

    //navigate up
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

    //navigate down
    pub fn select_down(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if self.selected_list_idx < self.task_lists.len() - 1 {
                    self.selected_list_idx += 1;
                }
            }
            ActivePane::Tasks => {
                if self.selected_task_idx < self.tasks.len() - 1 {
                    self.selected_task_idx += 1;
                }
            }
        }
    }

    //switch focus between list of task_list and task inside the selected list
    pub fn switch_list(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::TaskLists => ActivePane::Tasks,
            ActivePane::Tasks => ActivePane::TaskLists,
        };
    }

    //select list and select task
    pub fn selected_list(&self) -> Option<&TaskList> {
        self.task_lists.get(self.selected_list_idx)
    }

    pub fn selected_task(&self) -> Option<&TaskLocal> {
        self.tasks.get(self.selected_task_idx)
    }

    pub fn selected_task_mut(&mut self) -> Option<&mut TaskLocal> {
        self.tasks.get_mut(self.selected_task_idx)
    }

    // Action
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
        self.edit_action = EditAction::Create;
        self.input_buffer.clear();
        self.status_message =
            "Type task title and press Enter to save (Esc to cancel)...".to_string();
    }

    pub fn start_edit_task(&mut self) {
        if self.active_pane == ActivePane::Tasks {
            let existing_title = self
                .selected_task()
                .and_then(|t| t.title.clone())
                .unwrap_or_default();

            if self.selected_task().is_some() {
                self.input_mode = InputMode::Editing;
                self.edit_action = EditAction::EditTitle;
                self.input_buffer = existing_title;
                self.status_message =
                    "Edit task title and press Enter to save (Esc to cancel)...".to_string();
            }
        }
    }
    pub fn submit_input(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }

        match self.edit_action {
            EditAction::Create => {
                if let Some(current_list) = self.selected_list() {
                    let new_task = TaskLocal {
                        id: String::new(), // Will get official server ID upon sync or local uuid
                        list_id: current_list.id.clone(),
                        title: Some(self.input_buffer.trim().to_string()),
                        is_completed: false,
                        notes: None,
                        due: None,
                        completed: None,
                        parent: None,
                        updated: Some(chrono::Utc::now()),
                        is_dirty: true, // 👈 Mark dirty for sync!
                    };
                    self.tasks.push(new_task);
                    self.selected_task_idx = self.tasks.len() - 1;
                    self.status_message =
                        format!("Created new task '{}'", self.input_buffer.trim());
                }
            }
            EditAction::EditTitle => {
                let new_title = self.input_buffer.trim().to_string();
                if let Some(task) = self.selected_task_mut() {
                    task.title = Some(new_title.clone());
                    task.updated = Some(chrono::Utc::now());
                    task.is_dirty = true; // 👈 Mark dirty for sync!
                    self.status_message = format!("Updated task title to '{}'", new_title);
                }
            }
        }

        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
    }
}
