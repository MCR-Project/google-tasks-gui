use gtasks_core::api::{TaskList, TaskLocal};
use gtasks_core::db::Database;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;

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
            set_title: self.title.trim(),
            set_activatable: true,
            connect_activated[sender, id = self.id.clone()] => move |_| {
                let _ = sender.output(TaskListRowOutput::Select(id.clone()));
            }
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
            set_title: self.title.trim(),

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
            title: init.title.trim().to_string(),
            is_completed: init.is_completed,
        }
    }
}

struct AppModel {
    db: Option<Database>,
    list_id: String,
    list_title: String,
    task_lists: Vec<TaskList>,
    task_list_factory: FactoryVecDeque<TaskListRow>,
    tasks: FactoryVecDeque<TaskRow>,
    entry_buffer: gtk::EntryBuffer,
}

#[derive(Debug)]
enum AppInput {
    AddTask,
    ToggleTask(DynamicIndex),
    SelectTaskList(String),
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Google Tasks"),
            set_default_size: (900, 600),

            adw::Flap {
                #[wrap(Some)]
                set_flap = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_width_request: 220,

                    adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            set_title: "Task Lists",
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

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            #[watch]
                            set_title: model.list_title.trim(),
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
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let db_path = "task_lists.db";
        println!("[GUI DB LOG] Opening DB at: {:?}", db_path);

        let db = match Database::new(db_path) {
            Ok(db) => Some(db),
            Err(err) => {
                eprintln!("[GUI DB LOG] Error opening DB: {:?}", err);
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
                    println!("[GUI DB LOG] Lists found: {:?}", lists);
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
                        if let Err(err) = db.save_task_lists(&[default_list.clone()]) {
                            eprintln!("[GUI DB LOG] Error saving default list: {:?}", err);
                        }
                        task_lists = vec![default_list];
                    }
                }
                Err(err) => {
                    eprintln!("[GUI DB LOG] Error fetching task lists: {:?}", err);
                }
            }

            match db.get_tasks_for_list(&list_id) {
                Ok(tasks) => {
                    println!("[GUI DB LOG] Tasks found for list {}: {:?}", list_id, tasks);
                    initial_tasks = tasks;
                }
                Err(err) => {
                    eprintln!("[GUI DB LOG] Error fetching tasks for list {}: {:?}", list_id, err);
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
                TaskRowOutput::ToggleCompleted(index) => AppInput::ToggleTask(index),
            });

        {
            let mut guard = tasks.guard();
            guard.clear();
            for task in initial_tasks {
                let clean_title = task
                    .title
                    .as_deref()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .unwrap_or("Untitled Task")
                    .to_string();

                guard.push_back(TaskRowInit {
                    id: task.id,
                    title: clean_title,
                    is_completed: task.is_completed,
                });
            }
        }

        let entry_buffer = gtk::EntryBuffer::default();

        let model = AppModel {
            db,
            list_id,
            list_title,
            task_lists,
            task_list_factory,
            tasks,
            entry_buffer,
        };

        let sidebar_list_box = model.task_list_factory.widget();
        if let Some(first_row) = sidebar_list_box.row_at_index(0) {
            sidebar_list_box.select_row(Some(&first_row));
        }

        let task_list_box = model.tasks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
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
                            eprintln!("Failed to fetch tasks for list {}: {}", self.list_id, err);
                            Vec::new()
                        }
                    };

                    let mut guard = self.tasks.guard();
                    guard.clear();
                    for task in new_tasks {
                        let clean_title = task
                            .title
                            .as_deref()
                            .map(|t| t.trim())
                            .filter(|t| !t.is_empty())
                            .unwrap_or("Untitled Task")
                            .to_string();

                        guard.push_back(TaskRowInit {
                            id: task.id,
                            title: clean_title,
                            is_completed: task.is_completed,
                        });
                    }
                }
            }
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
                    let title = row.title.trim().to_string();

                    if let Some(ref db) = self.db {
                        let updated_task = TaskLocal {
                            id: task_id,
                            list_id: self.list_id.clone(),
                            title: Some(title),
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


