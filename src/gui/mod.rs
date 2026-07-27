use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use std::error::Error;

use crate::api::{GoogleTasksClient, TaskList, TaskLocal};
use crate::db::Database;

pub struct AppModel {
    client: GoogleTasksClient,
    db: Database,
    task_lists: Vec<TaskList>,
    tasks: Vec<TaskLocal>,
    selected_list_id: String,
    status_message: String,
}

#[derive(Debug)]
pub enum AppMsg {
    SelectList(String),
    ToggleTask(String, bool),
    CreateTask(String),
    CreateList(String),
    SyncCloud,
    SyncCompleted(Result<(), String>),
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
                            #[wrap(Some)]
                            set_child = &gtk::ListBox {
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
                            },

                            gtk::Button {
                                set_label: "Add Task",
                                add_css_class: "flat",
                            }
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[wrap(Some)]
                            set_child = &gtk::ListBox {
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
        let tasks = db.get_tasks_for_list(&first_id).unwrap_or_default();

        let model = AppModel {
            client,
            db,
            task_lists: lists,
            tasks,
            selected_list_id: first_id,
            status_message: "Ready".to_string(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::SelectList(list_id) => {
                self.selected_list_id = list_id.clone();
                if let Ok(tasks) = self.db.get_tasks_for_list(&list_id) {
                    self.tasks = tasks;
                }
            }
            AppMsg::ToggleTask(task_id, is_completed) => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.is_completed = is_completed;
                    task.updated = Some(chrono::Utc::now());
                    task.is_dirty = true;
                    let _ = self.db.save_tasks(&[task.clone()]);
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
                    self.tasks = tasks;
                }
            }
            AppMsg::CreateList(title) => {
                let client = self.client.clone();
                let db = self.db.clone();
                tokio::spawn(async move {
                    let mut client_mut = client;
                    if let Ok(new_list) = client_mut.create_task_list(&title).await {
                        let _ = db.save_task_lists(&[new_list]);
                    }
                });
            }
            AppMsg::SyncCloud => {
                let mut client = self.client.clone();
                let mut db = self.db.clone();
                tokio::spawn(async move {
                    let _ = crate::sync_local_to_db(&mut client, &mut db).await;
                    let _ = crate::sync_remote_to_db(&mut client, &mut db).await;
                    sender.input(AppMsg::SyncCompleted(Ok(())));
                });
            }
            AppMsg::SyncCompleted(_) => {
                if let Ok(tasks) = self.db.get_tasks_for_list(&self.selected_list_id) {
                    self.tasks = tasks;
                }
                if let Ok(lists) = self.db.get_task_lists() {
                    self.task_lists = lists;
                }
            }
        }
    }
}

/// Runs the native GTK4 & Libadwaita Linux Desktop Application
pub fn run(
    client: GoogleTasksClient,
    db: Database,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = RelmApp::new("com.gtasks.desktop");
    app.run::<AppModel>((client, db));
    Ok(())
}
