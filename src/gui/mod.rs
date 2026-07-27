use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use std::collections::HashSet;
use std::error::Error;
use chrono::Datelike;

use crate::api::{GoogleTasksClient, TaskList, TaskLocal};
use crate::db::Database;

fn load_css() {
    let provider = gtk::CssProvider::new();
    let css = r#"
/* Google Tasks Material Design 3 Tokens & Styling */
:root {
    --md-sys-color-background: #FFFFFF;
    --md-sys-color-surface: #FFFFFF;
    --md-sys-color-surface-variant: #F8F9FA;
    --md-sys-color-outline: rgba(0, 0, 0, 0.08);
    --md-sys-color-primary: #1A73E8;
    --md-sys-color-on-primary-container: #041E49;
    --md-sys-color-primary-container: #C2E7FF;
    --md-sys-color-on-surface: #1F1F1F;
    --md-sys-color-on-surface-variant: #444746;
}

window {
    background-color: #FFFFFF;
    color: #1F1F1F;
    font-family: "Google Sans", "Inter", "Cantarell", sans-serif;
}

.workspace {
    background-color: #F8F9FA;
}

.sidebar {
    background-color: #FFFFFF;
    border-right: 1px solid rgba(0, 0, 0, 0.08);
    padding: 8px;
}

/* Material 3 FAB Button (Pill Button 9999px) */
button.fab-button {
    background-color: #FFFFFF;
    color: #1F1F1F;
    border: 1px solid rgba(0, 0, 0, 0.12);
    border-radius: 9999px;
    padding: 10px 24px;
    box-shadow: 0px 1px 3px rgba(0, 0, 0, 0.08);
    font-weight: 500;
    transition: all 150ms ease;
}

button.fab-button:hover {
    background-color: rgba(0, 0, 0, 0.04);
    box-shadow: 0px 2px 6px rgba(0, 0, 0, 0.12);
}

/* Active Navigation Pill */
.navigation-sidebar row.pill-active, button.pill-active, button.suggested-action {
    background-color: #C2E7FF;
    color: #041E49;
    border-radius: 9999px;
    font-weight: 600;
    padding: 8px 16px;
    border: none;
    transition: all 150ms ease;
}

/* Material 3 Task Cards */
.task-card {
    background-color: #FFFFFF;
    border-radius: 16px;
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0px 1px 3px rgba(0, 0, 0, 0.04);
    padding: 12px;
    transition: all 150ms ease;
}

.card-title {
    font-size: 16px;
    font-weight: 600;
    color: #1F1F1F;
}

/* Interactive Rows & Hover States */
.task-row, listbox row {
    border-radius: 12px;
    padding: 4px 8px;
    transition: all 150ms ease;
}

listbox row:hover {
    background-color: rgba(0, 0, 0, 0.04);
}

/* Add Task Action Button */
button.add-task-btn {
    color: #1A73E8;
    font-weight: 500;
    border-radius: 9999px;
    padding: 6px 12px;
    transition: all 150ms ease;
}

button.add-task-btn:hover {
    background-color: rgba(26, 115, 232, 0.08);
}

/* Category Headers */
.heading {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: #444746;
    text-transform: uppercase;
}
    "#;
    provider.load_from_data(css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    AllTasks,
    Starred,
    Board,
}

#[derive(Debug)]
pub enum AppMsg {
    SetViewMode(ViewMode),
    ToggleListChecked(String, bool),
    SelectTask(String),
    ToggleTask(String, bool),
    ToggleTaskStar(String),
    CreateTaskForList(String, String),
    CreateSubtask(String),
    CreateList(String),
    DeleteList(String),
    UpdateDueDate(i32, i32, i32),
    ClearDueDate,
    SaveTaskDetails,
    SyncCloud,
    SyncCompleted(Result<(), String>),
}

#[derive(Clone, Debug)]
pub struct ListRowData {
    pub list: TaskList,
    pub count: usize,
    pub is_checked: bool,
}

pub struct ListRow {
    data: ListRowData,
}

#[derive(Debug)]
pub enum ListRowMsg {
    ToggleCheck(bool),
    Delete,
}

#[relm4::factory(pub)]
impl relm4::factory::FactoryComponent for ListRow {
    type Init = ListRowData;
    type Input = ListRowMsg;
    type Output = AppMsg;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_start: 4,
            set_margin_end: 4,

            gtk::CheckButton {
                set_active: self.data.is_checked,
                connect_toggled[sender] => move |btn| {
                    sender.input(ListRowMsg::ToggleCheck(btn.is_active()));
                }
            },

            gtk::Label {
                set_text: &self.data.list.title,
                set_hexpand: true,
                set_halign: gtk::Align::Start,
                set_ellipsize: gtk::pango::EllipsizeMode::End,
            },

            gtk::Label {
                set_text: &self.data.count.to_string(),
                add_css_class: "dim-label",
                set_visible: self.data.count > 0,
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
        Self { data: init }
    }

    fn update(&mut self, msg: Self::Input, sender: relm4::factory::FactorySender<Self>) {
        match msg {
            ListRowMsg::ToggleCheck(checked) => {
                sender.output(AppMsg::ToggleListChecked(self.data.list.id.clone(), checked)).unwrap();
            }
            ListRowMsg::Delete => {
                sender.output(AppMsg::DeleteList(self.data.list.id.clone())).unwrap();
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
                set_button: 1,
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

fn create_card_widget(list: &TaskList, tasks: &[TaskLocal], sender: ComponentSender<AppModel>) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_width_request(280);
    card.set_hexpand(true);
    card.set_margin_all(8);
    card.add_css_class("task-card");

    // 1. Header
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_margin_all(12);
    let title_label = gtk::Label::new(Some(&list.title));
    title_label.set_hexpand(true);
    title_label.set_halign(gtk::Align::Start);
    title_label.add_css_class("card-title");
    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("more-vertical-symbolic");
    menu_btn.add_css_class("flat");
    header.append(&title_label);
    header.append(&menu_btn);
    card.append(&header);

    // 2. Add Task Entry
    let entry_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    entry_box.set_margin_all(8);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Add a task"));
    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .css_classes(vec!["add-task-btn".to_string(), "flat".to_string()])
        .build();

    let list_id = list.id.clone();
    let sender_clone = sender.clone();
    let entry_clone = entry.clone();
    add_btn.connect_clicked(move |_| {
        let text = entry_clone.text().to_string();
        if !text.trim().is_empty() {
            sender_clone.input(AppMsg::CreateTaskForList(list_id.clone(), text));
            entry_clone.set_text("");
        }
    });

    let list_id_act = list.id.clone();
    let sender_act = sender.clone();
    let entry_act = entry.clone();
    entry.connect_activate(move |_| {
        let text = entry_act.text().to_string();
        if !text.trim().is_empty() {
            sender_act.input(AppMsg::CreateTaskForList(list_id_act.clone(), text));
            entry_act.set_text("");
        }
    });

    entry_box.append(&entry);
    entry_box.append(&add_btn);
    card.append(&entry_box);

    // 3. Uncompleted Tasks List
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_min_content_height(280);

    let list_box = gtk::ListBox::new();
    list_box.add_css_class("boxed-list");

    for task in tasks {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_all(6);
        let check = gtk::CheckButton::new();
        check.set_active(task.is_completed);
        let task_id = task.id.clone();
        let sender_toggle = sender.clone();
        check.connect_toggled(move |btn| {
            sender_toggle.input(AppMsg::ToggleTask(task_id.clone(), btn.is_active()));
        });

        let title_str = task.title.as_deref().unwrap_or("").strip_prefix("⭐ ").unwrap_or_else(|| task.title.as_deref().unwrap_or("")).to_string();
        let label = gtk::Label::new(Some(&title_str));
        label.set_halign(gtk::Align::Start);
        label.set_hexpand(true);

        let task_id_select = task.id.clone();
        let sender_select = sender.clone();
        let gesture = gtk::GestureClick::new();
        gesture.connect_pressed(move |_, _, _, _| {
            sender_select.input(AppMsg::SelectTask(task_id_select.clone()));
        });
        row.add_controller(gesture);

        row.append(&check);
        row.append(&label);
        list_box.append(&row);
    }

    scroll.set_child(Some(&list_box));
    card.append(&scroll);

    card
}

pub struct AppModel {
    client: GoogleTasksClient,
    db: Database,
    task_lists: FactoryVecDeque<ListRow>,
    tasks: FactoryVecDeque<TaskRow>,
    cards_box: gtk::Box,
    checked_list_ids: HashSet<String>,
    view_mode: ViewMode,
    all_tasks_count: usize,
    starred_tasks_count: usize,
    task_entry_buffer: gtk::EntryBuffer,
    list_entry_buffer: gtk::EntryBuffer,
    selected_task_id: Option<String>,
    task_title_buffer: gtk::EntryBuffer,
    task_notes_buffer: gtk::TextBuffer,
    task_due_date: Option<chrono::NaiveDate>,
    subtask_entry_buffer: gtk::EntryBuffer,
}

impl AppModel {
    fn reload_tasks(&mut self, sender: &ComponentSender<Self>) {
        let tasks = match &self.view_mode {
            ViewMode::AllTasks => {
                self.db.get_all_tasks().unwrap_or_default()
            }
            ViewMode::Starred => {
                if let Ok(all) = self.db.get_all_tasks() {
                    all.into_iter().filter(|t| t.title.as_deref().unwrap_or("").starts_with("⭐ ")).collect()
                } else {
                    Vec::new()
                }
            }
            ViewMode::Board => {
                if let Ok(all) = self.db.get_all_tasks() {
                    all.into_iter().filter(|t| self.checked_list_ids.contains(&t.list_id)).collect()
                } else {
                    Vec::new()
                }
            }
        };

        {
            let mut guard = self.tasks.guard();
            guard.clear();
            for task in tasks {
                guard.push_back(task);
            }
        }

        self.all_tasks_count = self.db.get_all_uncompleted_count().unwrap_or(0);
        self.starred_tasks_count = self.db.get_starred_uncompleted_count().unwrap_or(0);
        self.reload_cards(sender);
    }

    fn reload_cards(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.cards_box.first_child() {
            self.cards_box.remove(&child);
        }

        if let Ok(all_lists) = self.db.get_task_lists() {
            for list in all_lists {
                if self.checked_list_ids.contains(&list.id) {
                    let tasks = self.db.get_tasks_for_list(&list.id).unwrap_or_default();
                    let card = create_card_widget(&list, &tasks, sender.clone());
                    self.cards_box.append(&card);
                }
            }
        }
    }

    fn reload_lists(&mut self) {
        if let Ok(lists) = self.db.get_task_lists() {
            let mut guard = self.task_lists.guard();
            guard.clear();
            for list in &lists {
                let count = self.db.get_uncompleted_count_for_list(&list.id).unwrap_or(0);
                let is_checked = self.checked_list_ids.contains(&list.id);
                guard.push_back(ListRowData { list: list.clone(), count, is_checked });
            }
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
            set_default_width: 1280,
            set_default_height: 760,
            set_title: Some("Google Tasks — Full Material 3 Web Application"),

            #[wrap(Some)]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                // Top Application Bar
                adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_halign: gtk::Align::Center,

                        gtk::Image {
                            set_icon_name: Some("emblem-ok-symbolic"),
                            add_css_class: "accent",
                        },
                        gtk::Label {
                            set_text: "Tasks",
                            add_css_class: "title",
                        }
                    },

                    pack_start = &gtk::Button {
                        set_icon_name: "open-menu-symbolic",
                        add_css_class: "flat",
                    },

                    pack_end = &gtk::Button {
                        set_label: "🔄 Sync Cloud",
                        add_css_class: "suggested-action",
                        connect_clicked => AppMsg::SyncCloud,
                    },

                    pack_end = &gtk::Button {
                        set_icon_name: "help-about-symbolic",
                        add_css_class: "flat",
                    },

                    pack_end = &gtk::Button {
                        set_icon_name: "avatar-default-symbolic",
                        add_css_class: "flat",
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    // Left Navigation Sidebar (Strict 140px Width)
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 140,
                        set_hexpand: false,
                        add_css_class: "sidebar",

                        // FAB Create Button
                        gtk::Button {
                            add_css_class: "fab-button",
                            set_margin_all: 12,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 8,
                                set_halign: gtk::Align::Center,

                                gtk::Image {
                                    set_icon_name: Some("list-add-symbolic"),
                                },
                                gtk::Label {
                                    set_text: "Create",
                                    add_css_class: "bold",
                                }
                            }
                        },

                        // View Filters
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 4,
                            set_margin_start: 8,
                            set_margin_end: 8,

                            gtk::Button {
                                #[watch]
                                add_css_class: if model.view_mode == ViewMode::AllTasks { "suggested-action" } else { "flat" },
                                set_halign: gtk::Align::Fill,
                                connect_clicked => AppMsg::SetViewMode(ViewMode::AllTasks),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Image {
                                        set_icon_name: Some("emblem-ok-symbolic"),
                                    },
                                    gtk::Label {
                                        set_text: "All tasks",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                    },
                                    gtk::Label {
                                        set_text: &model.all_tasks_count.to_string(),
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_visible: model.all_tasks_count > 0,
                                    }
                                }
                            },

                            gtk::Button {
                                #[watch]
                                add_css_class: if model.view_mode == ViewMode::Starred { "suggested-action" } else { "flat" },
                                set_halign: gtk::Align::Fill,
                                connect_clicked => AppMsg::SetViewMode(ViewMode::Starred),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Image {
                                        set_icon_name: Some("non-starred-symbolic"),
                                    },
                                    gtk::Label {
                                        set_text: "Starred",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                    },
                                    gtk::Label {
                                        set_text: &model.starred_tasks_count.to_string(),
                                        add_css_class: "dim-label",
                                        #[watch]
                                        set_visible: model.starred_tasks_count > 0,
                                    }
                                }
                            }
                        },

                        gtk::Separator {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_top: 8,
                            set_margin_bottom: 8,
                        },

                        // Lists Section
                        gtk::Label {
                            set_text: "Lists",
                            set_halign: gtk::Align::Start,
                            set_margin_start: 16,
                            set_margin_end: 16,
                            set_margin_top: 4,
                            set_margin_bottom: 4,
                            add_css_class: "heading",
                        },

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            #[local_ref]
                            task_lists -> gtk::ListBox {
                                add_css_class: "navigation-sidebar",
                            }
                        },

                        // Create List Action
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 4,
                            set_margin_all: 8,

                            gtk::Entry {
                                set_hexpand: true,
                                set_placeholder_text: Some("Create new list..."),
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

                    // Main Board Workspace (Horizontal Card Grid or Single View)
                    gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        add_css_class: "workspace",

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            #[local_ref]
                            cards_box -> gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                set_margin_all: 16,
                                #[watch]
                                set_visible: model.view_mode == ViewMode::Board,
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_hexpand: true,
                                set_margin_all: 16,
                                set_spacing: 12,
                                #[watch]
                                set_visible: model.view_mode != ViewMode::Board,

                                gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    #[local_ref]
                                    tasks -> gtk::ListBox {
                                        add_css_class: "boxed-list",
                                    }
                                }
                            }
                        }
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                        #[watch]
                        set_visible: model.selected_task_id.is_some(),
                    },

                    // Right Sidebar Task Details
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
        load_css();

        let lists = db.get_task_lists().unwrap_or_default();
        let mut checked_list_ids = HashSet::new();
        if let Some(first) = lists.first() {
            checked_list_ids.insert(first.id.clone());
        }

        let initial_tasks = if let Some(first) = lists.first() {
            db.get_tasks_for_list(&first.id).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut task_lists = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |msg| msg);
        {
            let mut guard = task_lists.guard();
            for list in &lists {
                let count = db.get_uncompleted_count_for_list(&list.id).unwrap_or(0);
                let is_checked = checked_list_ids.contains(&list.id);
                guard.push_back(ListRowData { list: list.clone(), count, is_checked });
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

        let cards_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        cards_box.set_margin_all(16);

        let all_tasks_count = db.get_all_uncompleted_count().unwrap_or(0);
        let starred_tasks_count = db.get_starred_uncompleted_count().unwrap_or(0);

        let mut model = AppModel {
            client,
            db,
            task_lists,
            tasks: tasks_factory,
            cards_box,
            checked_list_ids,
            view_mode: ViewMode::Board,
            all_tasks_count,
            starred_tasks_count,
            task_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
            list_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
            selected_task_id: None,
            task_title_buffer: gtk::EntryBuffer::new(None::<&str>),
            task_notes_buffer: gtk::TextBuffer::new(None),
            task_due_date: None,
            subtask_entry_buffer: gtk::EntryBuffer::new(None::<&str>),
        };

        model.reload_cards(&sender);

        let task_lists = model.task_lists.widget();
        let tasks = model.tasks.widget();
        let cards_box = &model.cards_box;
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::SetViewMode(mode) => {
                self.view_mode = mode;
                self.selected_task_id = None;
                self.reload_tasks(&sender);
                self.reload_lists();
            }
            AppMsg::ToggleListChecked(list_id, checked) => {
                if checked {
                    self.checked_list_ids.insert(list_id);
                } else {
                    self.checked_list_ids.remove(&list_id);
                }
                self.view_mode = ViewMode::Board;
                self.reload_tasks(&sender);
                self.reload_lists();
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
                        self.reload_tasks(&sender);
                        self.reload_lists();
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

                        self.reload_tasks(&sender);
                        self.reload_lists();
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
            AppMsg::CreateTaskForList(list_id, title) => {
                if title.trim().is_empty() {
                    return;
                }
                let target_list_id = if !list_id.is_empty() {
                    list_id
                } else if let Some(first_checked) = self.checked_list_ids.iter().next() {
                    first_checked.clone()
                } else if let Ok(lists) = self.db.get_task_lists() {
                    lists.first().map(|l| l.id.clone()).unwrap_or_default()
                } else {
                    String::new()
                };

                if target_list_id.is_empty() {
                    return;
                }

                let new_task = TaskLocal {
                    id: String::new(),
                    list_id: target_list_id,
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
                self.reload_tasks(&sender);
                self.reload_lists();
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
                            self.reload_tasks(&sender);
                            self.reload_lists();
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
                            self.reload_tasks(&sender);
                            self.reload_lists();
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
                self.checked_list_ids.remove(&list_id);
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
                self.reload_lists();
                self.reload_tasks(&sender);
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
                self.reload_tasks(&sender);
                self.reload_lists();
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
