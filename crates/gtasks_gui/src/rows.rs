use gtasks_core::api::TaskLocal;
use gtasks_core::order_tasks_hierarchically;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

pub struct TaskListRow {
    pub id: String,
    pub title: String,
}

pub struct TaskListRowInit {
    pub id: String,
    pub title: String,
}

#[derive(Debug)]
pub enum TaskListRowOutput {
    Select(String),
}

#[relm4::factory(pub)]
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

pub struct TaskRow {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due_str: Option<String>,
    pub parent: Option<String>,
    pub is_subtask: bool,
    pub is_completed: bool,
}

pub struct TaskRowInit {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due_str: Option<String>,
    pub parent: Option<String>,
    pub is_subtask: bool,
    pub is_completed: bool,
}

#[derive(Debug)]
pub enum TaskRowOutput {
    ToggleCompleted(DynamicIndex),
    OpenDetails(String),
}

#[relm4::factory(pub)]
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

pub fn populate_task_guards(
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
