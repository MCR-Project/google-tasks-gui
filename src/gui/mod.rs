use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use std::error::Error;

use crate::api::{GoogleTasksClient, TaskList, TaskLocal};
use crate::db::Database;

#[derive(Debug)]
pub enum AppMsg {
    SelectList(String),
    ToggleTask(String, bool),
    CreateTask(String),
    CreateList(String),
    SyncCloud,
    SyncCompleted(Result<(), String>),
}

pub struct ListRow {
    list: TaskList,
}

#[derive(Debug)]
pub enum ListRowMsg {
    Select,
}

#[relm4::factory(pub)]
impl relm4::factory::FactoryComponent for ListRow {
    type Init = TaskList;
    type Input = ListRowMsg;
    type Output = AppMsg;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Button {
            set_label: &self.list.title,
            add_css_class: "flat",
            connect_clicked[sender] => move |_| {
                sender.input(ListRowMsg::Select);
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
        }
    }
}

pub struct TaskRow {
    task: TaskLocal,
}

#[derive(Debug)]
pub enum TaskRowMsg {
    Toggle(bool),
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
            
            gtk::CheckButton {
                set_active: self.task.is_completed,
                connect_toggled[sender] => move |btn| {
                    sender.input(TaskRowMsg::Toggle(btn.is_active()));
                }
            },
            gtk::Label {
                set_text: self.task.title.as_deref().unwrap_or(""),
                set_hexpand: true,
                set_halign: gtk::Align::Start,
                set_wrap: true,
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
        }
    }
}

pub struct AppModel {
    client: GoogleTasksClient,
    db: Database,
    task_lists: FactoryVecDeque<ListRow>,
    tasks: FactoryVecDeque<TaskRow>,
    selected_list_id: String,
    task_entry_buffer: gtk::EntryBuffer,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = (GoogleTasksClient, Database);
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_default_width: 960,
            set_default_height: 640,
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

                        gtk::Label {
                            set_text: "TASK LISTS",
                            set_halign: gtk::Align::Start,
                            set_margin_all: 12,
                            add_css_class: "caption",
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[local_ref]
                            task_lists -> gtk::ListBox {
                                add_css_class: "navigation-sidebar",
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

                        // Add Task Entry Bar
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,

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
            task_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
        };

        let task_lists = model.task_lists.widget();
        let tasks = model.tasks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::SelectList(list_id) => {
                self.selected_list_id = list_id.clone();
                if let Ok(tasks) = self.db.get_tasks_for_list(&list_id) {
                    let mut guard = self.tasks.guard();
                    guard.clear();
                    for task in tasks {
                        guard.push_back(task);
                    }
                }
            }
            AppMsg::ToggleTask(task_id, is_completed) => {
                if let Ok(tasks) = self.db.get_tasks_for_list(&self.selected_list_id) {
                    if let Some(mut task) = tasks.into_iter().find(|t| t.id == task_id) {
                        task.is_completed = is_completed;
                        task.updated = Some(chrono::Utc::now());
                        task.is_dirty = true;
                        let _ = self.db.save_tasks(&[task.clone()]);
                    }
                }
            }
            AppMsg::CreateTask(title) => {
                if title.trim().is_empty() || self.selected_list_id.is_empty() {
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
                if let Ok(tasks) = self.db.get_tasks_for_list(&self.selected_list_id) {
                    let mut guard = self.tasks.guard();
                    guard.clear();
                    for task in tasks {
                        guard.push_back(task);
                    }
                }
                self.task_entry_buffer.set_text("");
            }
            AppMsg::CreateList(title) => {
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
                if let Ok(tasks) = self.db.get_tasks_for_list(&self.selected_list_id) {
                    let mut guard = self.tasks.guard();
                    guard.clear();
                    for task in tasks {
                        guard.push_back(task);
                    }
                }
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
