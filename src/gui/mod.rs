use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use std::error::Error;
use chrono::Datelike;

use crate::api::{GoogleTasksClient, TaskList, TaskLocal};
use crate::db::Database;

#[derive(Debug)]
pub enum AppMsg {
    SelectList(String),
    ShowStarredTasks,
    SelectTask(String),
    ToggleTask(String, bool),
    ToggleTaskStar(String),
    CreateTask(String),
    CreateSubtask(String),
    CreateList(String),
    DeleteList(String),
    UpdateDueDate(i32, i32, i32),
    ClearDueDate,
    SaveTaskDetails,
    SyncCloud,
    SyncCompleted(Result<(), String>),
}

pub struct ListRow {
    list: TaskList,
}

#[derive(Debug)]
pub enum ListRowMsg {
    Select,
    Delete,
}

#[relm4::factory(pub)]
impl relm4::factory::FactoryComponent for ListRow {
    type Init = TaskList;
    type Input = ListRowMsg;
    type Output = AppMsg;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 4,

            gtk::Button {
                set_label: &self.list.title,
                add_css_class: "flat",
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
                connect_clicked[sender] => move |_| {
                    sender.input(ListRowMsg::Select);
                }
            },

            gtk::Button {
                set_icon_name: "user-trash-symbolic",
                add_css_class: "flat",
                connect_clicked[sender] => move |_| {
                    sender.input(ListRowMsg::Delete);
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: relm4::factory::FactorySender<Self>) -> Self {
        Self { list: init }
    }

    fn update(&mut self, msg: Self::Input, sender: relm4::factory::FactorySender<Self>) {
        match msg {
            ListRowMsg::Select => {
                sender.output(AppMsg::SelectList(self.list.id.clone())).unwrap();
            }
            ListRowMsg::Delete => {
                sender.output(AppMsg::DeleteList(self.list.id.clone())).unwrap();
            }
        }
    }
}

pub struct TaskRow {
    task: TaskLocal,
}

#[derive(Debug)]
pub enum TaskRowMsg {
    Toggle(bool),
    Select,
    ToggleStar,
}

#[relm4::factory(pub)]
impl relm4::factory::FactoryComponent for TaskRow {
    type Init = TaskLocal;
    type Input = TaskRowMsg;
    type Output = AppMsg;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 12,
            set_margin_all: 8,
            set_margin_start: if self.task.parent.as_ref().map(|p| !p.is_empty()).unwrap_or(false) { 32 } else { 8 },

            add_controller = gtk::GestureClick {
                set_button: 1, // left click
                connect_pressed[sender] => move |_, _, _, _| {
                    sender.input(TaskRowMsg::Select);
                }
            },

            gtk::Label {
                set_text: "↳",
                add_css_class: "dim-label",
                set_visible: self.task.parent.as_ref().map(|p| !p.is_empty()).unwrap_or(false),
            },

            gtk::CheckButton {
                set_active: self.task.is_completed,
                connect_toggled[sender] => move |btn| {
                    sender.input(TaskRowMsg::Toggle(btn.is_active()));
                }
            },
            gtk::Label {
                set_text: self.task.title.as_deref().unwrap_or("").strip_prefix("⭐ ").unwrap_or_else(|| self.task.title.as_deref().unwrap_or("")),
                set_hexpand: true,
                set_halign: gtk::Align::Start,
                set_wrap: true,
            },
            gtk::Button {
                set_label: if self.task.title.as_deref().unwrap_or("").starts_with("⭐ ") { "⭐" } else { "☆" },
                add_css_class: "flat",
                connect_clicked[sender] => move |_| {
                    sender.input(TaskRowMsg::ToggleStar);
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: relm4::factory::FactorySender<Self>) -> Self {
        Self { task: init }
    }

    fn update(&mut self, msg: Self::Input, sender: relm4::factory::FactorySender<Self>) {
        match msg {
            TaskRowMsg::Toggle(active) => {
                self.task.is_completed = active;
                sender.output(AppMsg::ToggleTask(self.task.id.clone(), active)).unwrap();
            }
            TaskRowMsg::Select => {
                sender.output(AppMsg::SelectTask(self.task.id.clone())).unwrap();
            }
            TaskRowMsg::ToggleStar => {
                sender.output(AppMsg::ToggleTaskStar(self.task.id.clone())).unwrap();
            }
        }
    }
}

pub struct AppModel {
    client: GoogleTasksClient,
    db: Database,
    task_lists: FactoryVecDeque<ListRow>,
    tasks: FactoryVecDeque<TaskRow>,
    selected_list_id: String,
    is_starred_view: bool,
    task_entry_buffer: gtk::EntryBuffer,
    list_entry_buffer: gtk::EntryBuffer,
    selected_task_id: Option<String>,
    task_title_buffer: gtk::EntryBuffer,
    task_notes_buffer: gtk::TextBuffer,
    task_due_date: Option<chrono::NaiveDate>,
    subtask_entry_buffer: gtk::EntryBuffer,
}

impl AppModel {
    fn reload_tasks(&mut self) {
        let mut guard = self.tasks.guard();
        guard.clear();

        let tasks = if self.is_starred_view {
            if let Ok(all) = self.db.get_all_tasks() {
                all.into_iter().filter(|t| t.title.as_deref().unwrap_or("").starts_with("⭐ ")).collect()
            } else {
                Vec::new()
            }
        } else {
            self.db.get_tasks_for_list(&self.selected_list_id).unwrap_or_default()
        };

        for task in tasks {
            guard.push_back(task);
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = (GoogleTasksClient, Database);
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_default_width: 1200,
            set_default_height: 720,
            set_title: Some("Google Tasks — Native Linux App"),

            #[wrap(Some)]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &gtk::Label {
                        set_text: "Google Tasks",
                        add_css_class: "title",
                    },

                    pack_end = &gtk::Button {
                        set_label: "🔄 Sync Cloud",
                        add_css_class: "suggested-action",
                        connect_clicked => AppMsg::SyncCloud,
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    // Left Sidebar
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 260,
                        add_css_class: "background",

                        gtk::Button {
                            set_label: "⭐ Starred",
                            add_css_class: "flat",
                            set_halign: gtk::Align::Start,
                            set_margin_all: 8,
                            connect_clicked => AppMsg::ShowStarredTasks,
                        },

                        gtk::Label {
                            set_text: "TASK LISTS",
                            set_halign: gtk::Align::Start,
                            set_margin_start: 12,
                            set_margin_end: 12,
                            set_margin_top: 8,
                            set_margin_bottom: 8,
                            add_css_class: "caption",
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[local_ref]
                            task_lists -> gtk::ListBox {
                                add_css_class: "navigation-sidebar",
                            }
                        },

                        // Add List Entry Bar
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 4,
                            set_margin_all: 8,

                            gtk::Entry {
                                set_hexpand: true,
                                set_placeholder_text: Some("New list..."),
                                set_buffer: &model.list_entry_buffer,
                                connect_activate[sender, buffer = model.list_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateList(buffer.text().to_string()));
                                }
                            },

                            gtk::Button {
                                set_icon_name: "list-add-symbolic",
                                add_css_class: "flat",
                                connect_clicked[sender, buffer = model.list_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateList(buffer.text().to_string()));
                                }
                            }
                        }
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                    },

                    // Main Tasks Grid
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,
                        set_margin_all: 16,
                        set_spacing: 12,

                        // Add Task Entry Bar (hidden in Starred view)
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            #[watch]
                            set_visible: !model.is_starred_view,

                            gtk::Entry {
                                set_hexpand: true,
                                set_placeholder_text: Some("Add a task..."),
                                set_buffer: &model.task_entry_buffer,
                                connect_activate[sender, buffer = model.task_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateTask(buffer.text().to_string()));
                                }
                            },

                            gtk::Button {
                                set_label: "Add Task",
                                add_css_class: "flat",
                                connect_clicked[sender, buffer = model.task_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateTask(buffer.text().to_string()));
                                }
                            }
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[local_ref]
                            tasks -> gtk::ListBox {
                                add_css_class: "boxed-list",
                            }
                        }
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                        #[watch]
                        set_visible: model.selected_task_id.is_some(),
                    },

                    // Right Sidebar for Task Details
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 320,
                        set_margin_all: 16,
                        set_spacing: 12,
                        #[watch]
                        set_visible: model.selected_task_id.is_some(),
                        add_css_class: "background",

                        gtk::Label {
                            set_text: "Task Details",
                            set_halign: gtk::Align::Start,
                            add_css_class: "title-4",
                        },

                        gtk::Label {
                            set_text: "Title",
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },
                        gtk::Entry {
                            set_buffer: &model.task_title_buffer,
                            set_placeholder_text: Some("Task Title"),
                        },

                        gtk::Label {
                            set_text: "Due Date",
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,

                            gtk::MenuButton {
                                set_hexpand: true,
                                #[watch]
                                set_label: &model.task_due_date
                                    .map(|d| format!("📅 {}", d.format("%Y-%m-%d")))
                                    .unwrap_or_else(|| "📅 Set Due Date".to_string()),
                                #[wrap(Some)]
                                set_popover = &gtk::Popover {
                                    #[wrap(Some)]
                                    set_child = &gtk::Calendar {
                                        set_halign: gtk::Align::Center,
                                        #[watch]
                                        select_day: &gtk::glib::DateTime::from_local(
                                            model.task_due_date.map(|d| d.year()).unwrap_or_else(|| chrono::Local::now().year()),
                                            model.task_due_date.map(|d| d.month() as i32).unwrap_or_else(|| chrono::Local::now().month() as i32),
                                            model.task_due_date.map(|d| d.day() as i32).unwrap_or_else(|| chrono::Local::now().day() as i32),
                                            0, 0, 0f64
                                        ).unwrap_or_else(|_| gtk::glib::DateTime::now_local().unwrap()),

                                        connect_day_selected[sender] => move |cal| {
                                            let date = cal.date();
                                            sender.input(AppMsg::UpdateDueDate(date.year(), date.month(), date.day_of_month()));
                                        }
                                    }
                                }
                            },

                            gtk::Button {
                                set_label: "Clear",
                                add_css_class: "flat",
                                connect_clicked => AppMsg::ClearDueDate,
                            }
                        },

                        gtk::Label {
                            set_text: "Notes",
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },
                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[wrap(Some)]
                            set_child = &gtk::TextView {
                                set_buffer: Some(&model.task_notes_buffer),
                                set_wrap_mode: gtk::WrapMode::WordChar,
                            }
                        },

                        gtk::Label {
                            set_text: "Add Subtask",
                            set_halign: gtk::Align::Start,
                            add_css_class: "caption",
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            gtk::Entry {
                                set_hexpand: true,
                                set_placeholder_text: Some("Subtask title..."),
                                set_buffer: &model.subtask_entry_buffer,
                                connect_activate[sender, buffer = model.subtask_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateSubtask(buffer.text().to_string()));
                                }
                            },
                            gtk::Button {
                                set_label: "Add",
                                add_css_class: "flat",
                                connect_clicked[sender, buffer = model.subtask_entry_buffer.clone()] => move |_| {
                                    sender.input(AppMsg::CreateSubtask(buffer.text().to_string()));
                                }
                            }
                        },

                        gtk::Button {
                            set_label: "Save Changes",
                            add_css_class: "suggested-action",
                            connect_clicked => AppMsg::SaveTaskDetails,
                        }
                    }
                }
            }
        }
    }

    fn init(
        (client, db): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let lists = db.get_task_lists().unwrap_or_default();
        let first_id = lists.first().map(|l| l.id.clone()).unwrap_or_default();
        let initial_tasks = db.get_tasks_for_list(&first_id).unwrap_or_default();

        let mut task_lists = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |msg| msg);
        {
            let mut guard = task_lists.guard();
            for list in lists {
                guard.push_back(list);
            }
        }

        let mut tasks_factory = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |msg| msg);
        {
            let mut guard = tasks_factory.guard();
            for task in initial_tasks {
                guard.push_back(task);
            }
        }

        let model = AppModel {
            client,
            db,
            task_lists,
            tasks: tasks_factory,
            selected_list_id: first_id,
            is_starred_view: false,
            task_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
            list_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
            selected_task_id: None,
            task_title_buffer: gtk::EntryBuffer::new(None::<&str>),
            task_notes_buffer: gtk::TextBuffer::new(None),
            task_due_date: None,
            subtask_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
        };

        let task_lists = model.task_lists.widget();
        let tasks = model.tasks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ShowStarredTasks => {
                self.is_starred_view = true;
                self.selected_list_id = String::new();
                self.selected_task_id = None;
                self.reload_tasks();
            }
            AppMsg::SelectList(list_id) => {
                self.is_starred_view = false;
                self.selected_list_id = list_id.clone();
                self.selected_task_id = None;
                self.reload_tasks();
            }
            AppMsg::SelectTask(task_id) => {
                self.selected_task_id = Some(task_id.clone());
                if let Ok(all_tasks) = self.db.get_all_tasks() {
                    if let Some(task) = all_tasks.into_iter().find(|t| t.id == task_id) {
                        self.task_title_buffer.set_text(task.title.as_deref().unwrap_or(""));
                        self.task_notes_buffer.set_text(task.notes.as_deref().unwrap_or(""));
                        if let Some(due) = task.due {
                            self.task_due_date = Some(due.date_naive());
                        } else {
                            self.task_due_date = None;
                        }
                    }
                }
            }
            AppMsg::ToggleTask(task_id, is_completed) => {
                if let Ok(all_tasks) = self.db.get_all_tasks() {
                    if let Some(mut task) = all_tasks.into_iter().find(|t| t.id == task_id) {
                        task.is_completed = is_completed;
                        task.updated = Some(chrono::Utc::now());
                        task.is_dirty = true;
                        let _ = self.db.save_tasks(&[task.clone()]);
                        self.reload_tasks();
                    }
                }
            }
            AppMsg::ToggleTaskStar(task_id) => {
                if let Ok(all_tasks) = self.db.get_all_tasks() {
                    if let Some(mut task) = all_tasks.into_iter().find(|t| t.id == task_id) {
                        let title = task.title.as_deref().unwrap_or("");
                        if title.starts_with("⭐ ") {
                            task.title = Some(title.strip_prefix("⭐ ").unwrap().to_string());
                        } else {
                            task.title = Some(format!("⭐ {}", title));
                        }
                        task.updated = Some(chrono::Utc::now());
                        task.is_dirty = true;
                        let _ = self.db.save_tasks(&[task.clone()]);

                        if Some(task_id) == self.selected_task_id {
                            self.task_title_buffer.set_text(task.title.as_deref().unwrap_or(""));
                        }

                        self.reload_tasks();
                    }
                }
            }
            AppMsg::UpdateDueDate(year, month, day) => {
                if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32) {
                    self.task_due_date = Some(date);
                }
            }
            AppMsg::ClearDueDate => {
                self.task_due_date = None;
            }
            AppMsg::CreateTask(title) => {
                if title.trim().is_empty() || self.selected_list_id.is_empty() || self.is_starred_view {
                    return;
                }
                let new_task = TaskLocal {
                    id: String::new(),
                    list_id: self.selected_list_id.clone(),
                    title: Some(title),
                    is_completed: false,
                    notes: None,
                    due: None,
                    completed: None,
                    parent: None,
                    updated: Some(chrono::Utc::now()),
                    is_dirty: true,
                };
                let _ = self.db.save_tasks(&[new_task]);
                self.reload_tasks();
                self.task_entry_buffer.set_text("");
            }
            AppMsg::CreateSubtask(title) => {
                if title.trim().is_empty() {
                    return;
                }
                if let Some(ref parent_id) = self.selected_task_id {
                    if let Ok(all_tasks) = self.db.get_all_tasks() {
                        if let Some(parent_task) = all_tasks.into_iter().find(|t| &t.id == parent_id) {
                            let new_task = TaskLocal {
                                id: String::new(),
                                list_id: parent_task.list_id.clone(),
                                title: Some(title),
                                is_completed: false,
                                notes: None,
                                due: None,
                                completed: None,
                                parent: Some(parent_id.clone()),
                                updated: Some(chrono::Utc::now()),
                                is_dirty: true,
                            };
                            let _ = self.db.save_tasks(&[new_task]);
                            self.reload_tasks();
                        }
                    }
                }
                self.subtask_entry_buffer.set_text("");
            }
            AppMsg::SaveTaskDetails => {
                if let Some(ref task_id) = self.selected_task_id {
                    if let Ok(all_tasks) = self.db.get_all_tasks() {
                        if let Some(mut task) = all_tasks.into_iter().find(|t| t.id == *task_id) {
                            task.title = Some(self.task_title_buffer.text().to_string());

                            let start = self.task_notes_buffer.start_iter();
                            let end = self.task_notes_buffer.end_iter();
                            let notes = self.task_notes_buffer.text(&start, &end, false).to_string();
                            task.notes = if notes.is_empty() { None } else { Some(notes) };

                            if let Some(due) = self.task_due_date {
                                task.due = due.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                            } else {
                                task.due = None;
                            }

                            task.updated = Some(chrono::Utc::now());
                            task.is_dirty = true;
                            let _ = self.db.save_tasks(&[task.clone()]);
                            self.reload_tasks();
                        }
                    }
                }
            }
            AppMsg::CreateList(title) => {
                if title.trim().is_empty() {
                    return;
                }
                let client = self.client.clone();
                let db = self.db.clone();
                let sender = sender.clone();
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                        rt.block_on(async {
                            let mut client_mut = client;
                            if let Ok(new_list) = client_mut.create_task_list(&title).await {
                                let _ = db.save_task_lists(&[new_list]);
                            }
                        });
                        sender.input(AppMsg::SyncCloud);
                    }
                });
                self.list_entry_buffer.set_text("");
            }
            AppMsg::DeleteList(list_id) => {
                let _ = self.db.delete_task_list_db(&list_id);
                let client = self.client.clone();
                let list_id_clone = list_id.clone();
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                        rt.block_on(async {
                            let mut client_mut = client;
                            let _ = client_mut.delete_task_list(&list_id_clone).await;
                        });
                    }
                });
                if self.selected_list_id == list_id {
                    self.selected_list_id = String::new();
                    self.selected_task_id = None;
                }
                if let Ok(lists) = self.db.get_task_lists() {
                    let mut guard = self.task_lists.guard();
                    guard.clear();
                    for list in &lists {
                        guard.push_back(list.clone());
                    }
                    if self.selected_list_id.is_empty() {
                        if let Some(first) = lists.first() {
                            self.selected_list_id = first.id.clone();
                        }
                    }
                }
                self.reload_tasks();
            }
            AppMsg::SyncCloud => {
                let mut client = self.client.clone();
                let mut db = self.db.clone();
                let sender = sender.clone();
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                        rt.block_on(async {
                            let _ = crate::sync_local_to_db(&mut client, &mut db).await;
                            let _ = crate::sync_remote_to_db(&mut client, &mut db).await;
                        });
                        sender.input(AppMsg::SyncCompleted(Ok(())));
                    }
                });
            }
            AppMsg::SyncCompleted(_) => {
                self.reload_tasks();
                if let Ok(lists) = self.db.get_task_lists() {
                    let mut guard = self.task_lists.guard();
                    guard.clear();
                    for list in lists {
                        guard.push_back(list);
                    }
                }
            }
        }
    }
}

pub fn run(
    client: GoogleTasksClient,
    db: Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = RelmApp::new("com.gtasks.desktop");
    app.run::<AppModel>((client, db));
    Ok(())
}
