use gtasks_core::api::{TaskList, TaskLocal};
use gtasks_core::db::Database;
use gtasks_core::parse_nlp_task;
use gtasks_core::sync::{SyncEvent, SyncManager};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;

use crate::rows::{
    populate_task_guards, TaskListRow, TaskListRowInit, TaskListRowOutput, TaskRow, TaskRowInit,
    TaskRowOutput,
};

pub struct AppModel {
    pub db: Option<Database>,
    pub sync_manager: Arc<SyncManager>,
    pub list_id: String,
    pub list_title: String,
    pub task_lists: Vec<TaskList>,
    pub task_list_factory: FactoryVecDeque<TaskListRow>,
    pub tasks: FactoryVecDeque<TaskRow>,
    pub completed_tasks: FactoryVecDeque<TaskRow>,
    pub entry_buffer: gtk::EntryBuffer,
    pub new_list_buffer: gtk::EntryBuffer,
    pub show_new_list_entry: bool,
    pub is_syncing: bool,
    pub editing_task_id: Option<String>,
    pub detail_title_buffer: gtk::EntryBuffer,
    pub detail_due_buffer: gtk::EntryBuffer,
    pub detail_notes_buffer: gtk::TextBuffer,
}

#[derive(Debug)]
pub enum AppInput {
    AddTask,
    ToggleActiveTask(DynamicIndex),
    MoveToCompleted(DynamicIndex),
    ToggleCompletedTask(DynamicIndex),
    DeleteTask(String),
    DeleteList(String),
    OpenTaskDetails(String),
    SaveTaskDetails,
    DeleteCurrentTaskDetails,
    DeleteCurrentList,
    SelectTaskList(String),
    ToggleNewListEntry,
    CreateTaskList,
    SyncWithGoogle,
    SetSyncing(bool),
    Refresh,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Google Tasks"),
            set_default_size: (1100, 650),
            connect_is_active_notify[sync_manager = model.sync_manager.clone()] => move |win| {
                sync_manager.set_window_active(win.is_active());
            },

            // Outer NavigationSplitView: Sidebar (Task Lists) vs Inner SplitView Container
            adw::NavigationSplitView {
                set_min_sidebar_width: 220.0,
                set_max_sidebar_width: 280.0,
                set_sidebar_width_fraction: 0.22,

                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: "Task Lists",
                    set_tag: Some("sidebar"),
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,

                        adw::HeaderBar {
                            set_show_start_title_buttons: false,
                            set_show_end_title_buttons: false,
                            #[wrap(Some)]
                            set_title_widget = &adw::WindowTitle {
                                set_title: "Task Lists",
                            },
                            pack_end = &gtk::Button {
                                set_icon_name: "list-add-symbolic",
                                set_tooltip_text: Some("Create new task list"),
                                connect_clicked => AppInput::ToggleNewListEntry,
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            #[watch]
                            set_visible: model.show_new_list_entry,

                            gtk::Entry {
                                set_placeholder_text: Some("New list name..."),
                                set_margin_all: 6,
                                set_buffer: &model.new_list_buffer,
                                connect_activate => AppInput::CreateTaskList,
                            },
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: true,

                            #[local_ref]
                            sidebar_list_box -> gtk::ListBox {
                                add_css_class: "navigation-sidebar",
                                set_margin_all: 6,
                            },
                        },
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    set_title: "Tasks",
                    set_tag: Some("content_container"),
                    #[wrap(Some)]
                    set_child = &adw::NavigationSplitView {
                        set_min_sidebar_width: 380.0,
                        set_max_sidebar_width: 650.0,
                        set_sidebar_width_fraction: 0.58,

                        // Inner NavigationSplitView Sidebar: Center Pane (Active Task List)
                        #[wrap(Some)]
                        set_sidebar = &adw::NavigationPage {
                            set_title: "Task List",
                            set_tag: Some("task_list"),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_hexpand: true,

                                adw::HeaderBar {
                                    set_show_start_title_buttons: false,
                                    set_show_end_title_buttons: false,
                                    #[wrap(Some)]
                                    set_title_widget = &adw::WindowTitle {
                                        #[watch]
                                        set_title: &relm4::gtk::glib::markup_escape_text(model.list_title.trim()),
                                    },
                                    pack_end = &gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 6,

                                        gtk::Spinner {
                                            #[watch]
                                            set_spinning: model.is_syncing,
                                            #[watch]
                                            set_visible: model.is_syncing,
                                        },

                                        gtk::Button {
                                            set_icon_name: "view-refresh-symbolic",
                                            set_tooltip_text: Some("Sync with Google Tasks"),
                                            #[watch]
                                            set_sensitive: !model.is_syncing,
                                            connect_clicked => AppInput::SyncWithGoogle,
                                        },

                                        gtk::Button {
                                            set_icon_name: "user-trash-symbolic",
                                            set_tooltip_text: Some("Delete active task list"),
                                            connect_clicked => AppInput::DeleteCurrentList,
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_start: 12,
                                    set_margin_end: 12,
                                    set_margin_top: 12,
                                    set_margin_bottom: 4,

                                    gtk::Entry {
                                        set_placeholder_text: Some("➕ Add task... (e.g. Buy milk tomorrow)"),
                                        set_buffer: &model.entry_buffer,
                                        connect_activate => AppInput::AddTask,
                                    },
                                },

                                gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    set_hexpand: true,

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_margin_all: 12,
                                        set_spacing: 12,

                                        #[local_ref]
                                        task_list_box -> gtk::ListBox {
                                            add_css_class: "boxed-list",
                                        },

                                        gtk::Expander {
                                            #[watch]
                                            set_label: Some(&format!("Completed ({})", model.completed_tasks.len())),
                                            #[watch]
                                            set_visible: !model.completed_tasks.is_empty(),

                                            #[local_ref]
                                            completed_task_list_box -> gtk::ListBox {
                                                add_css_class: "boxed-list",
                                                set_margin_top: 6,
                                            },
                                        },
                                    },
                                },
                            },
                        },

                        // Inner NavigationSplitView Content: Inspector Panel (Right Pane with Primary Window Controls)
                        #[wrap(Some)]
                        set_content = &adw::NavigationPage {
                            set_title: "Task Details",
                            set_tag: Some("inspector"),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_width_request: 320,

                                adw::HeaderBar {
                                    set_show_start_title_buttons: false,
                                    set_show_end_title_buttons: true,
                                    #[wrap(Some)]
                                    set_title_widget = &adw::WindowTitle {
                                        set_title: "Task Details",
                                    },
                                },

                                adw::PreferencesPage {
                                    adw::PreferencesGroup {
                                        set_title: "Task Information",

                                        adw::ActionRow {
                                            set_title: "Title",
                                            add_suffix = &gtk::Entry {
                                                set_buffer: &model.detail_title_buffer,
                                                set_valign: gtk::Align::Center,
                                                set_hexpand: true,
                                            },
                                        },

                                        adw::ActionRow {
                                            set_title: "Due Date",
                                            add_suffix = &gtk::Entry {
                                                set_placeholder_text: Some("YYYY-MM-DD"),
                                                set_buffer: &model.detail_due_buffer,
                                                set_valign: gtk::Align::Center,
                                                set_hexpand: true,
                                            },
                                        },
                                    },

                                    adw::PreferencesGroup {
                                        set_title: "Notes",

                                        gtk::ScrolledWindow {
                                            set_min_content_height: 120,
                                            set_margin_all: 6,

                                            gtk::TextView {
                                                set_buffer: Some(&model.detail_notes_buffer),
                                                set_wrap_mode: gtk::WrapMode::Word,
                                            },
                                        },
                                    },

                                    adw::PreferencesGroup {
                                        set_title: "Actions",

                                        adw::ActionRow {
                                            set_title: "Save Changes",
                                            add_suffix = &gtk::Button {
                                                set_label: "Save",
                                                add_css_class: "suggested-action",
                                                set_valign: gtk::Align::Center,
                                                connect_clicked => AppInput::SaveTaskDetails,
                                            },
                                        },

                                        adw::ActionRow {
                                            set_title: "Delete Task",
                                            add_suffix = &gtk::Button {
                                                set_label: "Delete",
                                                add_css_class: "destructive-action",
                                                set_valign: gtk::Align::Center,
                                                connect_clicked => AppInput::DeleteCurrentTaskDetails,
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let db_path = "task_lists.db";

        let db = match Database::new(db_path) {
            Ok(db) => Some(db),
            Err(err) => {
                tracing::error!("[GUI DB LOG] Error opening DB: {:?}", err);
                None
            }
        };

        let mut list_id = String::from("@default");
        let mut list_title = String::from("My Tasks");
        let mut task_lists = Vec::new();
        let mut initial_tasks = Vec::new();

        if let Some(ref db) = db {
            match db.get_task_lists() {
                Ok(lists) => {
                    if let Some(first_list) = lists.first() {
                        list_id = first_list.id.clone();
                        list_title = first_list.title.clone();
                        task_lists = lists;
                    } else {
                        let default_list = TaskList {
                            id: list_id.clone(),
                            title: list_title.clone(),
                            updated: None,
                        };
                        let _ = db.save_task_lists(std::slice::from_ref(&default_list));
                        task_lists = vec![default_list];
                    }
                }
                Err(err) => {
                    tracing::error!("Error fetching task lists: {:?}", err);
                }
            }

            match db.get_tasks_for_list(&list_id) {
                Ok(tasks) => {
                    initial_tasks = tasks;
                }
                Err(err) => {
                    tracing::error!("Error fetching tasks for list {}: {:?}", list_id, err);
                }
            }
        }

        let mut task_list_factory = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                TaskListRowOutput::Select(list_id) => AppInput::SelectTaskList(list_id),
            });

        {
            let mut guard = task_list_factory.guard();
            for list in &task_lists {
                guard.push_back(TaskListRowInit {
                    id: list.id.clone(),
                    title: list.title.trim().to_string(),
                });
            }
        }

        let mut tasks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                TaskRowOutput::ToggleCompleted(index) => AppInput::ToggleActiveTask(index),
                TaskRowOutput::OpenDetails(id) => AppInput::OpenTaskDetails(id),
            });

        let mut completed_tasks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                TaskRowOutput::ToggleCompleted(index) => AppInput::ToggleCompletedTask(index),
                TaskRowOutput::OpenDetails(id) => AppInput::OpenTaskDetails(id),
            });

        {
            let mut active_guard = tasks.guard();
            let mut completed_guard = completed_tasks.guard();
            populate_task_guards(initial_tasks, &mut active_guard, &mut completed_guard);
        }

        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(32);
        let sync_manager = SyncManager::spawn(event_tx);

        let sender_clone = sender.clone();
        relm4::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    SyncEvent::SyncStarted { is_manual } => {
                        if is_manual {
                            sender_clone.input(AppInput::SetSyncing(true));
                        }
                    }
                    SyncEvent::SyncFinished(_res) => {
                        sender_clone.input(AppInput::Refresh);
                    }
                }
            }
        });

        let entry_buffer = gtk::EntryBuffer::default();
        let new_list_buffer = gtk::EntryBuffer::default();
        let detail_title_buffer = gtk::EntryBuffer::default();
        let detail_due_buffer = gtk::EntryBuffer::default();
        let detail_notes_buffer = gtk::TextBuffer::default();

        let model = AppModel {
            db,
            sync_manager,
            list_id,
            list_title,
            task_lists,
            task_list_factory,
            tasks,
            completed_tasks,
            entry_buffer,
            new_list_buffer,
            show_new_list_entry: false,
            is_syncing: false,
            editing_task_id: None,
            detail_title_buffer,
            detail_due_buffer,
            detail_notes_buffer,
        };

        let sidebar_list_box = model.task_list_factory.widget();
        if let Some(first_row) = sidebar_list_box.row_at_index(0) {
            sidebar_list_box.select_row(Some(&first_row));
        }

        let task_list_box = model.tasks.widget();
        let completed_task_list_box = model.completed_tasks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppInput::OpenTaskDetails(id) => {
                self.editing_task_id = Some(id.clone());
                let mut title = String::new();
                let mut notes = String::new();
                let mut due_str = String::new();

                if let Some(ref db) = self.db {
                    if let Ok(tasks) = db.get_tasks_for_list(&self.list_id) {
                        if let Some(task) = tasks.iter().find(|t| t.id == id) {
                            title = task.title.as_deref().unwrap_or("").to_string();
                            notes = task.notes.as_deref().unwrap_or("").to_string();
                            due_str = task
                                .due
                                .map(|d| d.format("%Y-%m-%d").to_string())
                                .unwrap_or_default();
                        }
                    }
                }

                self.detail_title_buffer.set_text(&title);
                self.detail_due_buffer.set_text(&due_str);
                self.detail_notes_buffer.set_text(&notes);
            }
            AppInput::DeleteCurrentTaskDetails => {
                if let Some(id) = self.editing_task_id.take() {
                    self.detail_title_buffer.set_text("");
                    self.detail_due_buffer.set_text("");
                    self.detail_notes_buffer.set_text("");
                    sender.input(AppInput::DeleteTask(id));
                }
            }
            AppInput::DeleteCurrentList => {
                let list_id = self.list_id.clone();
                sender.input(AppInput::DeleteList(list_id));
            }
            AppInput::SaveTaskDetails => {
                if let Some(ref id) = self.editing_task_id.clone() {
                    let title_text = self.detail_title_buffer.text().to_string();
                    let title_trimmed = title_text.trim().to_string();
                    let notes_text = self
                        .detail_notes_buffer
                        .text(
                            &self.detail_notes_buffer.start_iter(),
                            &self.detail_notes_buffer.end_iter(),
                            false,
                        )
                        .to_string();
                    let notes_trimmed = notes_text.trim().to_string();
                    let due_text = self.detail_due_buffer.text().to_string();
                    let due_trimmed = due_text.trim();

                    let due_dt = chrono::NaiveDate::parse_from_str(due_trimmed, "%Y-%m-%d")
                        .ok()
                        .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                        .map(|dt| {
                            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                                dt,
                                chrono::Utc,
                            )
                        });

                    let mut is_completed_status = false;
                    let mut parent_id = None;

                    if let Some(ref db) = self.db {
                        if let Ok(tasks) = db.get_tasks_for_list(&self.list_id) {
                            if let Some(task) = tasks.iter().find(|t| t.id == *id) {
                                is_completed_status = task.is_completed;
                                parent_id = task.parent.clone();
                            }
                        }

                        let updated_task = TaskLocal {
                            id: id.clone(),
                            list_id: self.list_id.clone(),
                            title: if title_trimmed.is_empty() {
                                None
                            } else {
                                Some(title_trimmed.clone())
                            },
                            is_completed: is_completed_status,
                            notes: if notes_trimmed.is_empty() {
                                None
                            } else {
                                Some(notes_trimmed.clone())
                            },
                            due: due_dt,
                            completed: if is_completed_status {
                                Some(chrono::Utc::now())
                            } else {
                                None
                            },
                            parent: parent_id,
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                            is_deleted: false,
                        };

                        let db_clone = db.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Err(err) = db_clone.save_tasks(&[updated_task]) {
                                tracing::error!("Failed to save task details to SQLite: {}", err);
                            }
                        });
                    }

                    let due_str_formatted = due_dt.map(|d| d.format("%Y-%m-%d").to_string());
                    let notes_opt = if notes_trimmed.is_empty() {
                        None
                    } else {
                        Some(notes_trimmed)
                    };
                    let title_clean = if title_trimmed.is_empty() {
                        "Untitled Task".to_string()
                    } else {
                        title_trimmed
                    };

                    let active_pos = self.tasks.guard().iter().position(|r| r.id == *id);
                    if let Some(pos) = active_pos {
                        let mut guard = self.tasks.guard();
                        if let Some(row) = guard.get_mut(pos) {
                            row.title = title_clean.clone();
                            row.notes = notes_opt.clone();
                            row.due_str = due_str_formatted.clone();
                        }
                    }

                    let completed_pos =
                        self.completed_tasks.guard().iter().position(|r| r.id == *id);
                    if let Some(pos) = completed_pos {
                        let mut guard = self.completed_tasks.guard();
                        if let Some(row) = guard.get_mut(pos) {
                            row.title = title_clean;
                            row.notes = notes_opt;
                            row.due_str = due_str_formatted;
                        }
                    }

                    sender.input(AppInput::SyncWithGoogle);
                }
            }
            AppInput::DeleteTask(id) => {
                if let Some(ref db) = self.db {
                    let db_clone = db.clone();
                    let id_clone = id.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(err) = db_clone.mark_task_deleted(&id_clone) {
                            tracing::error!("Failed to mark task deleted in SQLite: {}", err);
                        }
                    });
                }

                let active_pos = self.tasks.guard().iter().position(|r| r.id == id);
                if let Some(pos) = active_pos {
                    self.tasks.guard().remove(pos);
                }

                let completed_pos = self.completed_tasks.guard().iter().position(|r| r.id == id);
                if let Some(pos) = completed_pos {
                    self.completed_tasks.guard().remove(pos);
                }

                sender.input(AppInput::SyncWithGoogle);
            }
            AppInput::DeleteList(list_id) => {
                if let Some(ref db) = self.db {
                    let db_clone = db.clone();
                    let list_id_clone = list_id.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(err) = db_clone.delete_task_list_db(&list_id_clone) {
                            tracing::error!("Failed to delete task list from SQLite: {}", err);
                        }
                    });
                }

                let pos = self
                    .task_list_factory
                    .guard()
                    .iter()
                    .position(|r| r.id == list_id);
                if let Some(p) = pos {
                    self.task_list_factory.guard().remove(p);
                }
                self.task_lists.retain(|l| l.id != list_id);

                let is_current = self.list_id == list_id;

                sender.input(AppInput::SyncWithGoogle);

                if is_current {
                    if let Some(first) = self.task_lists.first() {
                        let new_id = first.id.clone();
                        sender.input(AppInput::SelectTaskList(new_id));
                    } else {
                        let default_id = "@default".to_string();
                        let default_list = TaskList {
                            id: default_id.clone(),
                            title: "My Tasks".to_string(),
                            updated: None,
                        };
                        if let Some(ref db) = self.db {
                            let db_clone = db.clone();
                            let default_list_clone = default_list.clone();
                            tokio::task::spawn_blocking(move || {
                                let _ = db_clone.save_task_lists(std::slice::from_ref(&default_list_clone));
                            });
                        }
                        self.task_lists = vec![default_list];
                        sender.input(AppInput::SelectTaskList(default_id));
                    }
                }
            }
            AppInput::SyncWithGoogle => {
                self.sync_manager.trigger_sync();
            }
            AppInput::SetSyncing(is_syncing) => {
                self.is_syncing = is_syncing;
            }
            AppInput::Refresh => {
                self.is_syncing = false;

                if let Some(ref db) = self.db {
                    if let Ok(lists) = db.get_task_lists() {
                        self.task_lists = lists.clone();
                        let mut guard = self.task_list_factory.guard();
                        guard.clear();
                        for list in &lists {
                            guard.push_back(TaskListRowInit {
                                id: list.id.clone(),
                                title: list.title.trim().to_string(),
                            });
                        }

                        if !lists.iter().any(|l| l.id == self.list_id) {
                            if let Some(first) = lists.first() {
                                self.list_id = first.id.clone();
                                self.list_title = first.title.trim().to_string();
                            }
                        } else if let Some(found) = lists.iter().find(|l| l.id == self.list_id) {
                            self.list_title = found.title.trim().to_string();
                        }
                    }

                    let new_tasks = match db.get_tasks_for_list(&self.list_id) {
                        Ok(tasks) => tasks,
                        Err(err) => {
                            tracing::error!(
                                "Failed to fetch tasks on refresh for list {}: {}",
                                self.list_id, err
                            );
                            Vec::new()
                        }
                    };

                    let mut active_guard = self.tasks.guard();
                    let mut completed_guard = self.completed_tasks.guard();
                    populate_task_guards(new_tasks, &mut active_guard, &mut completed_guard);
                }
            }
            AppInput::ToggleNewListEntry => {
                self.show_new_list_entry = !self.show_new_list_entry;
            }
            AppInput::CreateTaskList => {
                let title = self.new_list_buffer.text().to_string();
                let trimmed = title.trim();
                if !trimmed.is_empty() {
                    let new_id = format!(
                        "list_{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                    );
                    let new_list = TaskList {
                        id: new_id.clone(),
                        title: trimmed.to_string(),
                        updated: Some(chrono::Utc::now().to_rfc3339()),
                    };

                    if let Some(ref db) = self.db {
                        let db_clone = db.clone();
                        let new_list_clone = new_list.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Err(err) = db_clone.save_task_lists(std::slice::from_ref(&new_list_clone)) {
                                tracing::error!("Failed to save new task list to SQLite: {}", err);
                            }
                        });
                    }

                    self.task_lists.push(new_list);
                    self.task_list_factory.guard().push_back(TaskListRowInit {
                        id: new_id.clone(),
                        title: trimmed.to_string(),
                    });

                    self.new_list_buffer.set_text("");
                    self.show_new_list_entry = false;

                    sender.input(AppInput::SelectTaskList(new_id));
                }
            }
            AppInput::SelectTaskList(selected_id) => {
                self.list_id = selected_id.clone();
                if let Some(list) = self.task_lists.iter().find(|l| l.id == selected_id) {
                    self.list_title = list.title.trim().to_string();
                } else if let Some(ref db) = self.db {
                    if let Ok(lists) = db.get_task_lists() {
                        self.task_lists = lists;
                        if let Some(list) = self.task_lists.iter().find(|l| l.id == selected_id) {
                            self.list_title = list.title.trim().to_string();
                        }
                    }
                }

                if let Some(ref db) = self.db {
                    let new_tasks = match db.get_tasks_for_list(&self.list_id) {
                        Ok(tasks) => tasks,
                        Err(err) => {
                            tracing::error!("Failed to fetch tasks for list {}: {}", self.list_id, err);
                            Vec::new()
                        }
                    };

                    let mut active_guard = self.tasks.guard();
                    let mut completed_guard = self.completed_tasks.guard();
                    populate_task_guards(new_tasks, &mut active_guard, &mut completed_guard);
                }
            }
            AppInput::AddTask => {
                let text = self.entry_buffer.text().to_string();
                let (clean_title, due_dt) = parse_nlp_task(&text);
                if !clean_title.is_empty() {
                    let task_id = format!(
                        "local_{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                    );

                    let task_local = TaskLocal {
                        id: task_id.clone(),
                        list_id: self.list_id.clone(),
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

                    if let Some(ref db) = self.db {
                        let db_clone = db.clone();
                        let task_local_clone = task_local.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Err(err) = db_clone.save_tasks(&[task_local_clone]) {
                                tracing::error!("Failed to save task to SQLite: {}", err);
                            }
                        });
                    }

                    let due_str_formatted = due_dt.map(|d| d.format("%Y-%m-%d").to_string());

                    self.tasks.guard().push_back(TaskRowInit {
                        id: task_id,
                        title: clean_title,
                        notes: None,
                        due_str: due_str_formatted,
                        parent: None,
                        is_subtask: false,
                        is_completed: false,
                    });

                    self.entry_buffer.set_text("");
                }
            }
            AppInput::ToggleActiveTask(index) => {
                let idx = index.current_index();
                let mut guard = self.tasks.guard();
                if let Some(row) = guard.get_mut(idx) {
                    row.is_completed = !row.is_completed;
                    let task_id = row.id.clone();
                    let is_completed = row.is_completed;
                    let title = row.title.trim().to_string();
                    let notes = row.notes.clone();
                    let parent = row.parent.clone();

                    let mut existing_due = None;
                    if let Some(ref db) = self.db {
                        if let Ok(tasks) = db.get_tasks_for_list(&self.list_id) {
                            if let Some(t) = tasks.iter().find(|t| t.id == task_id) {
                                existing_due = t.due;
                            }
                        }
                    }
                    if existing_due.is_none() {
                        existing_due = row.due_str.as_deref().and_then(|s| {
                            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                                .ok()
                                .and_then(|d| d.and_hms_opt(0, 0, 0))
                                .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
                        });
                    }

                    if let Some(ref db) = self.db {
                        let db_clone = db.clone();
                        let updated_task = TaskLocal {
                            id: task_id,
                            list_id: self.list_id.clone(),
                            title: Some(title),
                            is_completed,
                            notes,
                            due: existing_due,
                            completed: if is_completed {
                                Some(chrono::Utc::now())
                            } else {
                                None
                            },
                            parent,
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                            is_deleted: false,
                        };
                        tokio::task::spawn_blocking(move || {
                            if let Err(err) = db_clone.save_tasks(&[updated_task]) {
                                tracing::error!("Failed to update task completion in SQLite: {}", err);
                            }
                        });
                    }

                    if is_completed {
                        let sender_input = sender.input_sender().clone();
                        let idx_clone = index.clone();
                        relm4::gtk::glib::timeout_add_local(
                            std::time::Duration::from_secs(3),
                            move || {
                                let _ =
                                    sender_input.send(AppInput::MoveToCompleted(idx_clone.clone()));
                                relm4::gtk::glib::ControlFlow::Break
                            },
                        );
                    }
                }
            }
            AppInput::MoveToCompleted(index) => {
                let idx = index.current_index();
                let mut active_guard = self.tasks.guard();
                if let Some(row) = active_guard.get(idx) {
                    if row.is_completed {
                        let task_id = row.id.clone();
                        let title = row.title.clone();
                        let notes = row.notes.clone();
                        let due_str = row.due_str.clone();
                        let parent = row.parent.clone();
                        let is_subtask = row.is_subtask;
                        active_guard.remove(idx);

                        self.completed_tasks.guard().push_back(TaskRowInit {
                            id: task_id,
                            title,
                            notes,
                            due_str,
                            parent,
                            is_subtask,
                            is_completed: true,
                        });
                    }
                }
            }
            AppInput::ToggleCompletedTask(index) => {
                let idx = index.current_index();
                let mut completed_guard = self.completed_tasks.guard();
                if let Some(row) = completed_guard.get(idx) {
                    let task_id = row.id.clone();
                    let title = row.title.clone();
                    let notes = row.notes.clone();
                    let due_str = row.due_str.clone();
                    let parent = row.parent.clone();
                    let is_subtask = row.is_subtask;
                    completed_guard.remove(idx);

                    let mut existing_due = None;
                    if let Some(ref db) = self.db {
                        if let Ok(tasks) = db.get_tasks_for_list(&self.list_id) {
                            if let Some(t) = tasks.iter().find(|t| t.id == task_id) {
                                existing_due = t.due;
                            }
                        }
                    }
                    if existing_due.is_none() {
                        existing_due = due_str.as_deref().and_then(|s| {
                            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                                .ok()
                                .and_then(|d| d.and_hms_opt(0, 0, 0))
                                .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
                        });
                    }

                    if let Some(ref db) = self.db {
                        let db_clone = db.clone();
                        let updated_task = TaskLocal {
                            id: task_id.clone(),
                            list_id: self.list_id.clone(),
                            title: Some(title.clone()),
                            is_completed: false,
                            notes: notes.clone(),
                            due: existing_due,
                            completed: None,
                            parent: parent.clone(),
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                            is_deleted: false,
                        };
                        tokio::task::spawn_blocking(move || {
                            if let Err(err) = db_clone.save_tasks(&[updated_task]) {
                                tracing::error!("Failed to update task uncompleted state in SQLite: {}", err);
                            }
                        });
                    }

                    self.tasks.guard().push_back(TaskRowInit {
                        id: task_id,
                        title,
                        notes,
                        due_str,
                        parent,
                        is_subtask,
                        is_completed: false,
                    });
                }
            }
        }
    }
}
