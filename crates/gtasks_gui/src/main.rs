use chrono::Datelike;
use gtasks_core::api::{TaskList, TaskLocal};
use gtasks_core::db::Database;
use gtasks_core::sync::{SyncEvent, SyncManager};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;

struct TaskListRow {
    id: String,
    title: String,
}

struct TaskListRowInit {
    id: String,
    title: String,
}

#[derive(Debug)]
enum TaskListRowOutput {
    Select(String),
}

#[relm4::factory]
impl FactoryComponent for TaskListRow {
    type Init = TaskListRowInit;
    type Input = ();
    type Output = TaskListRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            #[watch]
            set_title: &relm4::gtk::glib::markup_escape_text(self.title.trim()),
            set_activatable: true,
            connect_activated[sender, id = self.id.clone()] => move |_| {
                let _ = sender.output(TaskListRowOutput::Select(id.clone()));
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        TaskListRow {
            id: init.id,
            title: init.title.trim().to_string(),
        }
    }
}

struct TaskRow {
    id: String,
    title: String,
    notes: Option<String>,
    due_str: Option<String>,
    parent: Option<String>,
    is_subtask: bool,
    is_completed: bool,
}

struct TaskRowInit {
    id: String,
    title: String,
    notes: Option<String>,
    due_str: Option<String>,
    parent: Option<String>,
    is_subtask: bool,
    is_completed: bool,
}

#[derive(Debug)]
enum TaskRowOutput {
    ToggleCompleted(DynamicIndex),
    OpenDetails(String),
}

#[relm4::factory]
impl FactoryComponent for TaskRow {
    type Init = TaskRowInit;
    type Input = ();
    type Output = TaskRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            #[watch]
            set_title: &relm4::gtk::glib::markup_escape_text(self.title.trim()),
            #[watch]
            set_subtitle: &relm4::gtk::glib::markup_escape_text(self.notes.as_deref().unwrap_or("")),
            set_activatable: true,
            set_margin_start: if self.is_subtask { 24 } else { 0 },
            #[watch]
            set_class_active: ("task-completed", self.is_completed),

            connect_activated[sender, id = self.id.clone()] => move |_| {
                let _ = sender.output(TaskRowOutput::OpenDetails(id.clone()));
            },

            add_prefix = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 4,

                gtk::Image {
                    set_icon_name: Some("corner-down-right-symbolic"),
                    set_visible: self.is_subtask,
                    set_opacity: 0.6,
                },

                gtk::CheckButton {
                    add_css_class: "task-check",
                    #[watch]
                    set_active: self.is_completed,
                    connect_toggled[sender, index] => move |_| {
                        let _ = sender.output(TaskRowOutput::ToggleCompleted(index.clone()));
                    }
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        TaskRow {
            id: init.id,
            title: init.title.trim().to_string(),
            notes: init.notes,
            due_str: init.due_str,
            parent: init.parent,
            is_subtask: init.is_subtask,
            is_completed: init.is_completed,
        }
    }
}

fn parse_nlp_task(input: &str) -> (String, Option<chrono::DateTime<chrono::Utc>>) {
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

fn order_tasks_hierarchically(tasks: Vec<TaskLocal>) -> Vec<TaskLocal> {
    let mut parent_map: std::collections::HashMap<Option<String>, Vec<TaskLocal>> =
        std::collections::HashMap::new();

    for task in tasks {
        parent_map.entry(task.parent.clone()).or_default().push(task);
    }

    let mut ordered = Vec::new();
    let root_tasks = parent_map.remove(&None).unwrap_or_default();

    fn add_with_children(
        task: TaskLocal,
        parent_map: &mut std::collections::HashMap<Option<String>, Vec<TaskLocal>>,
        ordered: &mut Vec<TaskLocal>,
    ) {
        let id = task.id.clone();
        ordered.push(task);
        if let Some(children) = parent_map.remove(&Some(id)) {
            for child in children {
                add_with_children(child, parent_map, ordered);
            }
        }
    }

    for root in root_tasks {
        add_with_children(root, &mut parent_map, &mut ordered);
    }

    for (_parent, orphans) in parent_map {
        for orphan in orphans {
            ordered.push(orphan);
        }
    }

    ordered
}

fn populate_task_guards(
    tasks: Vec<TaskLocal>,
    active_guard: &mut relm4::factory::FactoryVecDequeGuard<'_, TaskRow>,
    completed_guard: &mut relm4::factory::FactoryVecDequeGuard<'_, TaskRow>,
) {
    active_guard.clear();
    completed_guard.clear();

    let ordered_tasks = order_tasks_hierarchically(tasks);

    for task in ordered_tasks {
        let clean_title = task
            .title
            .as_deref()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .unwrap_or("Untitled Task")
            .to_string();

        let due_str = task.due.map(|d| d.format("%Y-%m-%d").to_string());
        let is_subtask = task.parent.is_some();

        if task.is_completed {
            completed_guard.push_back(TaskRowInit {
                id: task.id,
                title: clean_title,
                notes: task.notes,
                due_str,
                parent: task.parent,
                is_subtask,
                is_completed: true,
            });
        } else {
            active_guard.push_back(TaskRowInit {
                id: task.id,
                title: clean_title,
                notes: task.notes,
                due_str,
                parent: task.parent,
                is_subtask,
                is_completed: false,
            });
        }
    }
}

struct AppModel {
    db: Option<Database>,
    sync_manager: Arc<SyncManager>,
    list_id: String,
    list_title: String,
    task_lists: Vec<TaskList>,
    task_list_factory: FactoryVecDeque<TaskListRow>,
    tasks: FactoryVecDeque<TaskRow>,
    completed_tasks: FactoryVecDeque<TaskRow>,
    entry_buffer: gtk::EntryBuffer,
    new_list_buffer: gtk::EntryBuffer,
    show_new_list_entry: bool,
    is_syncing: bool,
    editing_task_id: Option<String>,
    detail_title_buffer: gtk::EntryBuffer,
    detail_due_buffer: gtk::EntryBuffer,
    detail_notes_buffer: gtk::TextBuffer,
}

#[derive(Debug)]
enum AppInput {
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

#[relm4::component]
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
                        .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
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

                        if let Err(err) = db.save_tasks(&[updated_task]) {
                            tracing::error!("Failed to save task details to SQLite: {}", err);
                        }
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
                    if let Err(err) = db.mark_task_deleted(&id) {
                        tracing::error!("Failed to mark task deleted in SQLite: {}", err);
                    }
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
                    if let Err(err) = db.delete_task_list_db(&list_id) {
                        tracing::error!("Failed to delete task list from SQLite: {}", err);
                    }
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

                let id_to_delete = list_id.clone();
                relm4::spawn(async move {
                    if !id_to_delete.starts_with("list_") {
                        if let Ok(mut client) = gtasks_core::obtain_authenticated_client().await {
                            let _ = client.delete_task_list(&id_to_delete).await;
                        }
                    }
                });

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
                            let _ = db.save_task_lists(std::slice::from_ref(&default_list));
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
                        if let Err(err) = db.save_task_lists(std::slice::from_ref(&new_list)) {
                            tracing::error!("Failed to save new task list to SQLite: {}", err);
                        }
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
                        if let Err(err) = db.save_tasks(&[task_local]) {
                            tracing::error!("Failed to save task to SQLite: {}", err);
                        }
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

                    if let Some(ref db) = self.db {
                        let updated_task = TaskLocal {
                            id: task_id,
                            list_id: self.list_id.clone(),
                            title: Some(title),
                            is_completed,
                            notes,
                            due: None,
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
                        if let Err(err) = db.save_tasks(&[updated_task]) {
                            tracing::error!("Failed to update task completion in SQLite: {}", err);
                        }
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

                    if let Some(ref db) = self.db {
                        let updated_task = TaskLocal {
                            id: task_id.clone(),
                            list_id: self.list_id.clone(),
                            title: Some(title.clone()),
                            is_completed: false,
                            notes: notes.clone(),
                            due: None,
                            completed: None,
                            parent: parent.clone(),
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                            is_deleted: false,
                        };
                        if let Err(err) = db.save_tasks(&[updated_task]) {
                            tracing::error!("Failed to update task uncompleted state in SQLite: {}", err);
                        }
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

fn main() {
    dotenvy::dotenv().ok();
    let app = RelmApp::new("com.example.gtasks");
    relm4::set_global_css(
        "
        .task-completed {
            text-decoration: line-through;
            opacity: 0.5;
            transition: opacity 300ms ease-in-out;
        }
        .task-check {
            min-width: 44px;
            min-height: 44px;
        }
        ",
    );
    app.run::<AppModel>(());
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
    fn test_parse_nlp_task_weekday() {
        let (title, due) = parse_nlp_task("Team sync next monday");
        assert_eq!(title, "Team sync");
        assert!(due.is_some());
    }

    #[test]
    fn test_parse_nlp_task_no_date() {
        let (title, due) = parse_nlp_task("Read documentation");
        assert_eq!(title, "Read documentation");
        assert!(due.is_none());
    }
}
