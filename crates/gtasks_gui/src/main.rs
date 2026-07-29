use gtasks_core::api::TaskLocal;
use gtasks_core::db::Database;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;

struct TaskRow {
    id: String,
    title: String,
    is_completed: bool,
}

struct TaskRowInit {
    id: String,
    title: String,
    is_completed: bool,
}

#[derive(Debug)]
enum TaskRowOutput {
    ToggleCompleted(DynamicIndex),
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
            set_title: &self.title,

            add_prefix = &gtk::CheckButton {
                #[watch]
                set_active: self.is_completed,
                connect_toggled[sender, index] => move |_| {
                    let _ = sender.output(TaskRowOutput::ToggleCompleted(index.clone()));
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        TaskRow {
            id: init.id,
            title: init.title,
            is_completed: init.is_completed,
        }
    }
}

struct AppModel {
    db: Option<Database>,
    list_id: String,
    list_title: String,
    tasks: FactoryVecDeque<TaskRow>,
    entry_buffer: gtk::EntryBuffer,
}

#[derive(Debug)]
enum AppInput {
    AddTask,
    ToggleTask(DynamicIndex),
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Google Tasks"),
            set_default_size: (800, 600),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        #[watch]
                        set_title: &model.list_title,
                    },
                },

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hexpand: true,

                    #[local_ref]
                    task_list_box -> gtk::ListBox {
                        add_css_class: "boxed-list",
                        set_margin_all: 12,
                    },
                },

                gtk::Entry {
                    set_placeholder_text: Some("New task..."),
                    set_margin_all: 12,
                    set_buffer: &model.entry_buffer,
                    connect_activate => AppInput::AddTask,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let db = Database::new("task_lists.db").ok();

        let mut list_id = String::from("@default");
        let mut list_title = String::from("My Tasks");

        let mut initial_tasks = Vec::new();

        if let Some(ref db) = db {
            if let Ok(lists) = db.get_task_lists() {
                if let Some(first_list) = lists.first() {
                    list_id = first_list.id.clone();
                    list_title = first_list.title.clone();
                }
            }

            if let Ok(tasks) = db.get_tasks_for_list(&list_id) {
                initial_tasks = tasks;
            } else if let Ok(all_tasks) = db.get_all_tasks() {
                initial_tasks = all_tasks;
            }
        }

        let mut tasks = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                TaskRowOutput::ToggleCompleted(index) => AppInput::ToggleTask(index),
            });

        {
            let mut guard = tasks.guard();
            for task in initial_tasks {
                guard.push_back(TaskRowInit {
                    id: task.id,
                    title: task.title.unwrap_or_else(|| "Untitled Task".to_string()),
                    is_completed: task.is_completed,
                });
            }
        }

        let entry_buffer = gtk::EntryBuffer::default();

        let model = AppModel {
            db,
            list_id,
            list_title,
            tasks,
            entry_buffer,
        };

        let task_list_box = model.tasks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppInput::AddTask => {
                let text = self.entry_buffer.text().to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let task_id = format!("local_{}", std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis());

                    let task_local = TaskLocal {
                        id: task_id.clone(),
                        list_id: self.list_id.clone(),
                        title: Some(trimmed.to_string()),
                        is_completed: false,
                        notes: None,
                        due: None,
                        completed: None,
                        parent: None,
                        updated: Some(chrono::Utc::now()),
                        is_dirty: true,
                    };

                    if let Some(ref db) = self.db {
                        if let Err(err) = db.save_tasks(&[task_local]) {
                            eprintln!("Failed to save task to SQLite: {}", err);
                        }
                    }

                    self.tasks.guard().push_back(TaskRowInit {
                        id: task_id,
                        title: trimmed.to_string(),
                        is_completed: false,
                    });

                    self.entry_buffer.set_text("");
                }
            }
            AppInput::ToggleTask(index) => {
                let idx = index.current_index();
                let mut guard = self.tasks.guard();
                if let Some(row) = guard.get_mut(idx) {
                    row.is_completed = !row.is_completed;
                    let task_id = row.id.clone();
                    let is_completed = row.is_completed;

                    if let Some(ref db) = self.db {
                        let updated_task = TaskLocal {
                            id: task_id,
                            list_id: self.list_id.clone(),
                            title: Some(row.title.clone()),
                            is_completed,
                            notes: None,
                            due: None,
                            completed: if is_completed { Some(chrono::Utc::now()) } else { None },
                            parent: None,
                            updated: Some(chrono::Utc::now()),
                            is_dirty: true,
                        };
                        if let Err(err) = db.save_tasks(&[updated_task]) {
                            eprintln!("Failed to update task completion in SQLite: {}", err);
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("com.example.gtasks");
    app.run::<AppModel>(());
}
