#![windows_subsystem = "windows"]

use chrono::{Local, Utc};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, Rect, Response, RichText, Sense, Stroke, Vec2,
    WindowLevel,
};
use gtasks_core::{
    obtain_authenticated_client, order_tasks_hierarchically, parse_nlp_task, sync_local_to_db,
    sync_remote_to_db, Database, GoogleTasksClient, TaskList, TaskLocal,
};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use uuid::Uuid;

const COLOR_BG: Color32 = Color32::from_rgb(13, 14, 18);
const COLOR_CARD: Color32 = Color32::from_rgb(19, 21, 27);
const COLOR_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 26);
const COLOR_ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(238, 240, 246);
const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(140, 146, 162);
const COLOR_TEXT_COMPLETED: Color32 = Color32::from_rgb(85, 90, 105);

const COLOR_TAG_RED_BG: Color32 = Color32::from_rgba_unmultiplied_const(220, 60, 60, 28);
const COLOR_TAG_RED_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(220, 60, 60, 80);
const COLOR_TAG_RED_TEXT: Color32 = Color32::from_rgb(248, 113, 113);

const COLOR_TAG_AMBER_BG: Color32 = Color32::from_rgba_unmultiplied_const(240, 160, 50, 28);
const COLOR_TAG_AMBER_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(240, 160, 50, 80);
const COLOR_TAG_AMBER_TEXT: Color32 = Color32::from_rgb(251, 191, 36);

const COLOR_TAG_BLUE_BG: Color32 = Color32::from_rgba_unmultiplied_const(100, 150, 250, 24);
const COLOR_TAG_BLUE_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(100, 150, 250, 70);
const COLOR_TAG_BLUE_TEXT: Color32 = Color32::from_rgb(147, 197, 253);

const MAP_PIN: &[u8] = include_bytes!("../assets/icons/map-pin.svg");
const RELOAD: &[u8] = include_bytes!("../assets/icons/reload.svg");
const CLOSE: &[u8] = include_bytes!("../assets/icons/close.svg");
const MINUS: &[u8] = include_bytes!("../assets/icons/minus.svg");
const CHECK: &[u8] = include_bytes!("../assets/icons/check.svg");
const PLUS: &[u8] = include_bytes!("../assets/icons/plus.svg");
const TRASH: &[u8] = include_bytes!("../assets/icons/trash.svg");
const CALENDAR: &[u8] = include_bytes!("../assets/icons/calendar.svg");
const CLOCK: &[u8] = include_bytes!("../assets/icons/clock.svg");
const SQUARE_ALERT: &[u8] = include_bytes!("../assets/icons/square-alert.svg");
const SQUARE: &[u8] = include_bytes!("../assets/icons/square.svg");
const COPY: &[u8] = include_bytes!("../assets/icons/copy.svg");
const CORNER_DOWN_RIGHT: &[u8] = include_bytes!("../assets/icons/corner-down-right.svg");
const EXTERNAL_LINK: &[u8] = include_bytes!("../assets/icons/external-link.svg");

fn icon(name: &'static str, bytes: &'static [u8], size: f32, tint: Color32) -> egui::Image<'static> {
    egui::Image::from_bytes(format!("bytes://{name}.svg"), bytes)
        .fit_to_exact_size(Vec2::splat(size))
        .tint(tint)
}

#[cfg(windows)]
mod desktop_pin {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

    pub type HWND = *mut c_void;
    type BOOL = i32;

    pub const GWLP_WNDPROC: i32 = -4;
    pub const GWLP_HWNDPARENT: i32 = -8;
    pub const GWL_EXSTYLE: i32 = -20;

    pub const WS_EX_TOOLWINDOW: isize = 0x00000080;
    pub const WS_EX_APPWINDOW: isize = 0x00040000;

    pub const HWND_BOTTOM: HWND = 1 as HWND;
    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOMOVE: u32 = 0x0002;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    pub const SWP_FRAMECHANGED: u32 = 0x0020;
    pub const SWP_HIDEWINDOW: u32 = 0x0080;

    pub const WM_WINDOWPOSCHANGING: u32 = 0x0046;
    pub const WM_SYSCOMMAND: u32 = 0x0112;
    pub const SC_MINIMIZE: usize = 0xF020;

    #[repr(C)]
    struct WINDOWPOS {
        hwnd: HWND,
        hwnd_insert_after: HWND,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    }

    static OLD_WNDPROC: AtomicIsize = AtomicIsize::new(0);
    static PIN_MODE_DESKTOP: AtomicBool = AtomicBool::new(true);
    static ALLOW_MANUAL_MINIMIZE: AtomicBool = AtomicBool::new(false);

    extern "system" {
        fn FindWindowA(class: *const u8, title: *const u8) -> HWND;
        fn FindWindowExA(parent: HWND, child_after: HWND, class: *const u8, title: *const u8) -> HWND;
        fn GetShellWindow() -> HWND;
        fn GetWindowLongPtrA(hwnd: HWND, index: i32) -> isize;
        fn SetWindowLongPtrA(hwnd: HWND, index: i32, new_long: isize) -> isize;
        fn SetWindowPos(hwnd: HWND, insert_after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> BOOL;
        fn EnumWindows(lp_enum_func: unsafe extern "system" fn(HWND, isize) -> BOOL, l_param: isize) -> BOOL;
        fn GetWindowThreadProcessId(hwnd: HWND, lp_dw_process_id: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: HWND) -> BOOL;
        fn GetWindowTextA(hwnd: HWND, lp_string: *mut u8, n_max_count: i32) -> i32;
        fn CallWindowProcA(prev_wnd_func: *mut c_void, hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn DefWindowProcA(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize;
    }

    unsafe extern "system" fn widget_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        if PIN_MODE_DESKTOP.load(Ordering::Relaxed) {
            // Block minimize caused by Win+D, Win+M, Show Desktop
            if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_MINIMIZE {
                if !ALLOW_MANUAL_MINIMIZE.load(Ordering::Relaxed) {
                    return 0;
                }
            }
            // Prevent Win+D / Show Desktop from hiding our window
            if msg == WM_WINDOWPOSCHANGING {
                let wp = lparam as *mut WINDOWPOS;
                if !wp.is_null() && !ALLOW_MANUAL_MINIMIZE.load(Ordering::Relaxed) {
                    (*wp).flags &= !SWP_HIDEWINDOW;
                }
            }
        }

        let old = OLD_WNDPROC.load(Ordering::Relaxed);
        if old != 0 {
            CallWindowProcA(old as *mut c_void, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcA(hwnd, msg, wparam, lparam)
        }
    }

    unsafe extern "system" fn find_own_window_callback(hwnd: HWND, lparam: isize) -> BOOL {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() && IsWindowVisible(hwnd) != 0 {
            let mut title = [0u8; 64];
            let len = GetWindowTextA(hwnd, title.as_mut_ptr(), title.len() as i32);
            if len > 0 {
                let out = lparam as *mut HWND;
                *out = hwnd;
                return 0;
            }
        }
        1
    }

    pub fn get_own_hwnd() -> Option<HWND> {
        unsafe {
            let mut my_hwnd: HWND = std::ptr::null_mut();
            EnumWindows(find_own_window_callback, &mut my_hwnd as *mut HWND as isize);
            if my_hwnd.is_null() {
                None
            } else {
                Some(my_hwnd)
            }
        }
    }

    pub fn anchor_to_desktop() -> bool {
        unsafe {
            let Some(my_hwnd) = get_own_hwnd() else { return false; };

            let mut shell = GetShellWindow();
            if shell.is_null() {
                shell = FindWindowA(b"Progman\0".as_ptr(), std::ptr::null());
            }

            let mut host = shell;
            if !shell.is_null() {
                let def_view = FindWindowExA(shell, std::ptr::null_mut(), b"SHELLDLL_DefView\0".as_ptr(), std::ptr::null());
                if def_view.is_null() {
                    let mut worker: HWND = std::ptr::null_mut();
                    loop {
                        worker = FindWindowExA(std::ptr::null_mut(), worker, b"WorkerW\0".as_ptr(), std::ptr::null());
                        if worker.is_null() {
                            break;
                        }
                        let dv = FindWindowExA(worker, std::ptr::null_mut(), b"SHELLDLL_DefView\0".as_ptr(), std::ptr::null());
                        if !dv.is_null() {
                            host = worker;
                            break;
                        }
                    }
                }
            }

            if host.is_null() {
                return false;
            }

            PIN_MODE_DESKTOP.store(true, Ordering::Relaxed);

            // Subclass window procedure to protect against Win+D minimization
            if OLD_WNDPROC.load(Ordering::Relaxed) == 0 {
                let old = SetWindowLongPtrA(my_hwnd, GWLP_WNDPROC, widget_wndproc as *const () as isize);
                OLD_WNDPROC.store(old, Ordering::Relaxed);
            }

            // Set desktop shell owner
            SetWindowLongPtrA(my_hwnd, GWLP_HWNDPARENT, host as isize);

            // Hide from Alt+Tab by setting WS_EX_TOOLWINDOW and clearing WS_EX_APPWINDOW
            let ex = GetWindowLongPtrA(my_hwnd, GWL_EXSTYLE);
            SetWindowLongPtrA(my_hwnd, GWL_EXSTYLE, (ex | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW);

            SetWindowPos(
                my_hwnd,
                HWND_BOTTOM,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            eprintln!("[desktop_pin] Anchored window {:?} to desktop host {:?}", my_hwnd, host);
            true
        }
    }

    pub fn send_to_bottom() {
        unsafe {
            if let Some(my_hwnd) = get_own_hwnd() {
                SetWindowPos(
                    my_hwnd,
                    HWND_BOTTOM,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }

    pub fn unanchor_from_desktop() {
        unsafe {
            PIN_MODE_DESKTOP.store(false, Ordering::Relaxed);
            if let Some(my_hwnd) = get_own_hwnd() {
                SetWindowLongPtrA(my_hwnd, GWLP_HWNDPARENT, 0);

                // Restore to Alt+Tab and taskbar
                let ex = GetWindowLongPtrA(my_hwnd, GWL_EXSTYLE);
                SetWindowLongPtrA(my_hwnd, GWL_EXSTYLE, (ex & !WS_EX_TOOLWINDOW) | WS_EX_APPWINDOW);

                SetWindowPos(
                    my_hwnd,
                    std::ptr::null_mut(),
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
                eprintln!("[desktop_pin] Unanchored window {:?}", my_hwnd);
            }
        }
    }

    pub fn allow_manual_minimize() {
        ALLOW_MANUAL_MINIMIZE.store(true, Ordering::Relaxed);
    }

    pub fn reset_manual_minimize() {
        ALLOW_MANUAL_MINIMIZE.store(false, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PinMode {
    Desktop,
    AlwaysOnTop,
    Normal,
}

impl PinMode {
    fn window_level(&self) -> WindowLevel {
        match self {
            PinMode::Desktop => WindowLevel::Normal,
            PinMode::AlwaysOnTop => WindowLevel::AlwaysOnTop,
            PinMode::Normal => WindowLevel::Normal,
        }
    }

    fn next(&self) -> Self {
        match self {
            PinMode::Desktop => PinMode::AlwaysOnTop,
            PinMode::AlwaysOnTop => PinMode::Normal,
            PinMode::Normal => PinMode::Desktop,
        }
    }

    fn tooltip(&self) -> &'static str {
        match self {
            PinMode::Desktop => "Pinned to Desktop (Widget mode: stays on desktop screen behind apps). Click for Always on Top.",
            PinMode::AlwaysOnTop => "Pinned: Always on Top (Floats above all windows). Click for Normal window.",
            PinMode::Normal => "Unpinned: Normal Window. Click to pin to Desktop.",
        }
    }

    fn is_active(&self) -> bool {
        matches!(self, PinMode::Desktop | PinMode::AlwaysOnTop)
    }
}

enum SyncEvent {
    SyncStarted,
    SyncCompleted,
    SyncFailed(String),
}

struct TasksWidgetApp {
    db: Database,
    task_lists: Vec<TaskList>,
    selected_list_index: usize,
    tasks: Vec<TaskLocal>,
    new_task_text: String,
    pin_mode: PinMode,
    desktop_anchored: bool,
    was_focused: bool,
    manually_minimized: bool,
    is_syncing: bool,
    sync_status: String,
    sync_sender: Sender<()>,
    sync_receiver: Receiver<SyncEvent>,
    collapsed_parents: HashSet<String>,
    adding_subtask_to: Option<String>,
    new_subtask_text: String,
}

struct DisplayTask {
    task: TaskLocal,
    depth: usize,
    has_children: bool,
    subtask_count: usize,
    parent_title: Option<String>,
}

fn build_display_tree(
    tasks: &[TaskLocal],
    collapsed_parents: &HashSet<String>,
) -> (Vec<DisplayTask>, Vec<DisplayTask>) {
    let task_map: HashMap<&str, &TaskLocal> = tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    let mut children_map: HashMap<Option<&str>, Vec<&TaskLocal>> = HashMap::new();
    for t in tasks {
        children_map.entry(t.parent.as_deref()).or_default().push(t);
    }

    let pending_ids: HashSet<&str> = tasks
        .iter()
        .filter(|t| !t.is_completed)
        .map(|t| t.id.as_str())
        .collect();

    fn collect_pending(
        task: &TaskLocal,
        depth: usize,
        task_map: &HashMap<&str, &TaskLocal>,
        children_map: &HashMap<Option<&str>, Vec<&TaskLocal>>,
        collapsed_parents: &HashSet<String>,
        result: &mut Vec<DisplayTask>,
    ) {
        let pending_children: Vec<&TaskLocal> = children_map
            .get(&Some(task.id.as_str()))
            .map(|children| children.iter().copied().filter(|c| !c.is_completed).collect())
            .unwrap_or_default();

        let parent_title = if depth == 0 {
            task.parent
                .as_deref()
                .and_then(|pid| task_map.get(pid))
                .and_then(|p| p.title.clone())
        } else {
            None
        };

        result.push(DisplayTask {
            task: task.clone(),
            depth,
            has_children: !pending_children.is_empty(),
            subtask_count: pending_children.len(),
            parent_title,
        });

        if !collapsed_parents.contains(&task.id) {
            for child in pending_children {
                collect_pending(child, depth + 1, task_map, children_map, collapsed_parents, result);
            }
        }
    }

    let mut pending_tree = Vec::new();
    for task in tasks.iter().filter(|t| !t.is_completed) {
        let is_root_pending = match &task.parent {
            None => true,
            Some(pid) => !pending_ids.contains(pid.as_str()),
        };
        if is_root_pending {
            collect_pending(task, 0, &task_map, &children_map, collapsed_parents, &mut pending_tree);
        }
    }

    let mut completed_tree = Vec::new();
    for task in tasks.iter().filter(|t| t.is_completed) {
        let parent_title = task
            .parent
            .as_deref()
            .and_then(|pid| task_map.get(pid))
            .and_then(|p| p.title.clone());

        completed_tree.push(DisplayTask {
            task: task.clone(),
            depth: if task.parent.is_some() { 1 } else { 0 },
            has_children: false,
            subtask_count: 0,
            parent_title,
        });
    }

    (pending_tree, completed_tree)
}

impl TasksWidgetApp {
    fn new(
        db: Database,
        sync_sender: Sender<()>,
        sync_receiver: Receiver<SyncEvent>,
    ) -> Self {
        let mut app = Self {
            db,
            task_lists: Vec::new(),
            selected_list_index: 0,
            tasks: Vec::new(),
            new_task_text: String::new(),
            pin_mode: PinMode::Desktop,
            desktop_anchored: false,
            was_focused: false,
            manually_minimized: false,
            is_syncing: false,
            sync_status: "Synced".to_string(),
            sync_sender,
            sync_receiver,
            collapsed_parents: HashSet::new(),
            adding_subtask_to: None,
            new_subtask_text: String::new(),
        };
        app.reload_data();
        app
    }

    fn reload_data(&mut self) {
        if let Ok(lists) = self.db.get_task_lists() {
            self.task_lists = lists;
            if self.selected_list_index >= self.task_lists.len() && !self.task_lists.is_empty() {
                self.selected_list_index = 0;
            }
        }
        self.reload_tasks();
    }

    fn reload_tasks(&mut self) {
        if let Some(list) = self.task_lists.get(self.selected_list_index) {
            if let Ok(tasks) = self.db.get_tasks_for_list(&list.id) {
                self.tasks = order_tasks_hierarchically(tasks);
            }
        } else {
            self.tasks.clear();
        }
    }

    fn selected_list_id(&self) -> Option<String> {
        self.task_lists
            .get(self.selected_list_index)
            .map(|l| l.id.clone())
    }

    fn add_task(&mut self) {
        let text = self.new_task_text.trim();
        if text.is_empty() {
            return;
        }

        let Some(list_id) = self.selected_list_id() else {
            return;
        };

        let (clean_title, due) = parse_nlp_task(text);
        let task = TaskLocal {
            id: format!("task_{}", Uuid::new_v4().simple()),
            list_id,
            title: Some(clean_title),
            is_completed: false,
            notes: None,
            due,
            completed: None,
            parent: None,
            updated: Some(Utc::now()),
            is_dirty: true,
            is_deleted: false,
        };

        if self.db.save_tasks(&[task]).is_ok() {
            self.new_task_text.clear();
            self.reload_tasks();
            let _ = self.sync_sender.send(());
        }
    }

    fn add_subtask(&mut self, parent_id: &str, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            self.adding_subtask_to = None;
            return;
        }

        let Some(list_id) = self.selected_list_id() else {
            return;
        };

        let (clean_title, due) = parse_nlp_task(text);
        let task = TaskLocal {
            id: format!("task_{}", Uuid::new_v4().simple()),
            list_id,
            title: Some(clean_title),
            is_completed: false,
            notes: None,
            due,
            completed: None,
            parent: Some(parent_id.to_string()),
            updated: Some(Utc::now()),
            is_dirty: true,
            is_deleted: false,
        };

        if self.db.save_tasks(&[task]).is_ok() {
            self.new_subtask_text.clear();
            self.adding_subtask_to = None;
            self.collapsed_parents.remove(parent_id);
            self.reload_tasks();
            let _ = self.sync_sender.send(());
        }
    }

    fn toggle_task(&mut self, task_id: &str) {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            let mut task = self.tasks[pos].clone();
            task.is_completed = !task.is_completed;
            task.completed = if task.is_completed {
                Some(Utc::now())
            } else {
                None
            };
            task.is_dirty = true;
            task.updated = Some(Utc::now());

            if self.db.save_tasks(&[task]).is_ok() {
                self.reload_tasks();
                let _ = self.sync_sender.send(());
            }
        }
    }

    fn delete_task(&mut self, task_id: &str) {
        if self.db.mark_task_deleted(task_id).is_ok() {
            self.reload_tasks();
            let _ = self.sync_sender.send(());
        }
    }

    fn trigger_sync(&mut self) {
        let _ = self.sync_sender.send(());
    }

    fn poll_sync_events(&mut self) {
        while let Ok(event) = self.sync_receiver.try_recv() {
            match event {
                SyncEvent::SyncStarted => {
                    self.is_syncing = true;
                    self.sync_status = "SYNCING".to_string();
                }
                SyncEvent::SyncCompleted => {
                    self.is_syncing = false;
                    self.sync_status = format!("SYNCED {}", Local::now().format("%H:%M"));
                    self.reload_data();
                }
                SyncEvent::SyncFailed(err) => {
                    self.is_syncing = false;
                    self.sync_status = format!("ERR: {err}");
                }
            }
        }
    }
}

fn pixel_checkbox(ui: &mut egui::Ui, checked: &mut bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
    if response.clicked() {
        *checked = !*checked;
    }

    let (border_color, fill_color) = if *checked || response.hovered() {
        (COLOR_ACCENT, if *checked { COLOR_ACCENT } else { Color32::from_white_alpha(30) })
    } else {
        (Color32::from_white_alpha(80), Color32::from_rgb(22, 24, 30))
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), fill_color);
    painter.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0, border_color),
        egui::StrokeKind::Inside,
    );

    if *checked {
        icon("check", CHECK, 12.0, Color32::WHITE).paint_at(ui, rect.shrink(2.0));
    }

    response
}

fn pixel_icon_btn(
    ui: &mut egui::Ui,
    name: &'static str,
    bytes: &'static [u8],
    size: f32,
    active: bool,
    tooltip: &str,
) -> Response {
    let btn_size = Vec2::splat(size + 10.0);
    let (rect, response) = ui.allocate_exact_size(btn_size, Sense::click());

    if response.hovered() {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_white_alpha(36));
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, Color32::from_white_alpha(80)),
            egui::StrokeKind::Inside,
        );
    }

    let tint = if active {
        COLOR_ACCENT
    } else if response.hovered() {
        Color32::WHITE
    } else {
        Color32::from_rgb(230, 235, 245)
    };

    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(size));
    icon(name, bytes, size, tint).paint_at(ui, icon_rect);

    response.on_hover_text(tooltip)
}

fn is_url_token(token: &str) -> bool {
    let clean = token.trim_matches(&['(', ')', '[', ']', '{', '}', '<', '>', '"', '\'', ',', ';', '.'][..]);
    clean.starts_with("http://")
        || clean.starts_with("https://")
        || clean.starts_with("www.")
        || [".com", ".org", ".dev", ".io", ".in", ".net", ".app", ".edu", ".gov"]
            .iter()
            .any(|tld| clean.contains(tld))
}

fn clean_url_for_open(url: &str) -> String {
    let clean = url.trim_matches(&['(', ')', '[', ']', '{', '}', '<', '>', '"', '\'', ',', ';', '.'][..]);
    if clean.starts_with("http://") || clean.starts_with("https://") {
        clean.to_string()
    } else {
        format!("https://{clean}")
    }
}

fn extract_first_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|w| is_url_token(w))
        .map(clean_url_for_open)
}

fn open_url_in_browser(raw_url: &str) {
    let target = clean_url_for_open(raw_url);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &target])
            .creation_flags(0x08000000)
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&target).spawn();
    }
}

fn render_formatted_notes(
    ui: &mut egui::Ui,
    notes: &str,
    default_color: Color32,
    is_completed: bool,
) {
    let trimmed = notes.trim();
    if trimmed.is_empty() {
        return;
    }

    ui.add_space(2.0);
    egui::Frame::default()
        .fill(Color32::from_white_alpha(7))
        .corner_radius(CornerRadius::same(3))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(15)))
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
            for line in trimmed.lines() {
                let line_trimmed = line.trim();
                if line_trimmed.is_empty() {
                    continue;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
                    for word in line_trimmed.split_whitespace() {
                        if is_url_token(word) {
                            let target_url = clean_url_for_open(word);
                            let mut link_text = RichText::new(word)
                                .font(FontId::proportional(11.0))
                                .underline();
                            link_text = if is_completed {
                                link_text.color(COLOR_TEXT_COMPLETED).strikethrough()
                            } else {
                                link_text.color(COLOR_ACCENT)
                            };
                            let resp = ui.add(
                                egui::Label::new(link_text)
                                    .wrap()
                                    .sense(Sense::click()),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                open_url_in_browser(&target_url);
                            }
                            resp.on_hover_text(format!("Open link in browser:\n{target_url}"));
                        } else {
                            let mut label_text = RichText::new(word)
                                .font(FontId::proportional(11.0))
                                .color(default_color);
                            if is_completed {
                                label_text = label_text.strikethrough();
                            }
                            ui.add(egui::Label::new(label_text).wrap());
                        }
                    }
                });
            }
        });
}

fn handle_window_resize(ui: &mut egui::Ui) {
    if ui.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }

    let rect = ui.max_rect();
    let Some(pos) = ui.input(|i| i.pointer.latest_pos()) else { return; };
    let border = 7.0;
    let corner = 14.0;

    let dir = match (
        pos.x <= rect.left() + corner,
        pos.x >= rect.right() - corner,
        pos.y <= rect.top() + corner,
        pos.y >= rect.bottom() - corner,
        pos.x <= rect.left() + border,
        pos.x >= rect.right() - border,
        pos.y <= rect.top() + border,
        pos.y >= rect.bottom() - border,
    ) {
        (_, true, _, true, _, _, _, _) => (egui::ResizeDirection::SouthEast, egui::CursorIcon::ResizeSouthEast),
        (true, _, _, true, _, _, _, _) => (egui::ResizeDirection::SouthWest, egui::CursorIcon::ResizeSouthWest),
        (_, true, true, _, _, _, _, _) => (egui::ResizeDirection::NorthEast, egui::CursorIcon::ResizeNorthEast),
        (true, _, true, _, _, _, _, _) => (egui::ResizeDirection::NorthWest, egui::CursorIcon::ResizeNorthWest),
        (_, _, _, _, _, true, _, _) => (egui::ResizeDirection::East, egui::CursorIcon::ResizeEast),
        (_, _, _, _, true, _, _, _) => (egui::ResizeDirection::West, egui::CursorIcon::ResizeWest),
        (_, _, _, _, _, _, _, true) => (egui::ResizeDirection::South, egui::CursorIcon::ResizeSouth),
        (_, _, _, _, _, _, true, _) => (egui::ResizeDirection::North, egui::CursorIcon::ResizeNorth),
        _ => return,
    };

    ui.ctx().set_cursor_icon(dir.1);
    if ui.input(|i| i.pointer.any_pressed()) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(dir.0));
    }
}

fn render_due_badge(ui: &mut egui::Ui, due: chrono::DateTime<Utc>) {
    let local_due = due.with_timezone(&Local);
    let now = Local::now();

    let (icon_name, icon_bytes, label, bg, border, text_col) =
        if local_due.date_naive() < now.date_naive() {
            (
                "square-alert",
                SQUARE_ALERT,
                format!("Overdue {}", local_due.format("%b %d")),
                COLOR_TAG_RED_BG,
                COLOR_TAG_RED_BORDER,
                COLOR_TAG_RED_TEXT,
            )
        } else if local_due.date_naive() == now.date_naive() {
            (
                "clock",
                CLOCK,
                "Today".to_string(),
                COLOR_TAG_AMBER_BG,
                COLOR_TAG_AMBER_BORDER,
                COLOR_TAG_AMBER_TEXT,
            )
        } else if local_due.date_naive() == now.date_naive() + chrono::Duration::days(1) {
            (
                "calendar",
                CALENDAR,
                "Tomorrow".to_string(),
                COLOR_TAG_BLUE_BG,
                COLOR_TAG_BLUE_BORDER,
                COLOR_TAG_BLUE_TEXT,
            )
        } else {
            (
                "calendar",
                CALENDAR,
                local_due.format("%b %d").to_string(),
                Color32::from_white_alpha(10),
                COLOR_BORDER,
                COLOR_TEXT_MUTED,
            )
        };

    egui::Frame::default()
        .fill(bg)
        .corner_radius(CornerRadius::same(3))
        .stroke(Stroke::new(1.0, border))
        .inner_margin(Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(icon(icon_name, icon_bytes, 10.0, text_col));
                ui.label(RichText::new(label).font(FontId::monospace(10.0)).color(text_col));
            });
        });
}

fn render_task_item(
    ui: &mut egui::Ui,
    item: &DisplayTask,
    collapsed_parents: &mut HashSet<String>,
    is_completed: bool,
    toggled_id: &mut Option<String>,
    deleted_id: &mut Option<String>,
    start_subtask_for: &mut Option<String>,
) {
    ui.horizontal_top(|ui| {
        if item.depth > 0 {
            ui.add_space((item.depth as f32) * 18.0);
            let color = if is_completed { COLOR_TEXT_COMPLETED } else { COLOR_TEXT_MUTED };
            ui.add(icon("corner-down-right", CORNER_DOWN_RIGHT, 12.0, color));
        }

        let mut checked = is_completed;
        if pixel_checkbox(ui, &mut checked).clicked() {
            *toggled_id = Some(item.task.id.clone());
        }

        let title = item.task.title.as_deref().unwrap_or("Untitled");
        let notes_text = item.task.notes.as_deref().unwrap_or("");
        let title_url = extract_first_url(title);
        let notes_url = extract_first_url(notes_text);
        let row_url = title_url.clone().or_else(|| notes_url.clone());

        let mut reserved_right = 30.0;
        if row_url.is_some() {
            reserved_right += 24.0;
        }
        if !is_completed && item.depth == 0 {
            reserved_right += 24.0;
        }

        let content_width = (ui.available_width() - reserved_right).max(60.0);
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                if let Some(ref p_title) = item.parent_title {
                    let text_col = if is_completed { COLOR_TEXT_COMPLETED } else { COLOR_TEXT_MUTED };
                    ui.label(
                        RichText::new(format!("Subtask of: {p_title}"))
                            .font(FontId::monospace(9.0))
                            .color(text_col),
                    );
                }

                ui.horizontal_wrapped(|ui| {
                    if let Some(ref url) = title_url {
                        let mut link_text = RichText::new(title)
                            .font(FontId::proportional(13.0))
                            .underline();
                        if is_completed {
                            link_text = link_text.color(COLOR_TEXT_COMPLETED).strikethrough();
                        } else {
                            link_text = link_text.color(COLOR_ACCENT);
                        }
                        let resp = ui.add(egui::Label::new(link_text).sense(Sense::click()));
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            open_url_in_browser(url);
                        }
                        resp.on_hover_text(format!("Click to open in browser:\n{url}"));
                    } else {
                        let mut label_text = RichText::new(title).font(FontId::proportional(13.0));
                        if is_completed {
                            label_text = label_text.color(COLOR_TEXT_COMPLETED).strikethrough();
                        } else {
                            label_text = label_text.color(COLOR_TEXT_PRIMARY);
                        }
                        ui.add(egui::Label::new(label_text).wrap());
                    }

                    if !is_completed && item.has_children {
                        let is_collapsed = collapsed_parents.contains(&item.task.id);
                        let arrow = if is_collapsed { "▸" } else { "▾" };
                        let badge_text = format!("{arrow} {} subtasks", item.subtask_count);
                        let btn = ui.add(
                            egui::Label::new(
                                RichText::new(badge_text)
                                    .font(FontId::monospace(9.0))
                                    .color(COLOR_ACCENT),
                            )
                            .sense(Sense::click()),
                        );
                        if btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if btn.clicked() {
                            if is_collapsed {
                                collapsed_parents.remove(&item.task.id);
                            } else {
                                collapsed_parents.insert(item.task.id.clone());
                            }
                        }
                    }
                });

                if !is_completed {
                    if let Some(due) = item.task.due {
                        render_due_badge(ui, due);
                    }
                }

                let notes_color = if is_completed { COLOR_TEXT_COMPLETED } else { COLOR_TEXT_MUTED };
                render_formatted_notes(ui, notes_text, notes_color, is_completed);
            },
        );

        if let Some(ref url) = row_url {
            let tooltip = format!("Open link in browser:\n{url}");
            if pixel_icon_btn(ui, "external-link", EXTERNAL_LINK, 12.0, false, &tooltip).clicked() {
                open_url_in_browser(url);
            }
        }

        if !is_completed && item.depth == 0 {
            if pixel_icon_btn(ui, "plus", PLUS, 11.0, false, "Add subtask").clicked() {
                *start_subtask_for = Some(item.task.id.clone());
            }
        }

        if pixel_icon_btn(ui, "trash", TRASH, 12.0, false, "Delete task").clicked() {
            *deleted_id = Some(item.task.id.clone());
        }
    });
}

impl eframe::App for TasksWidgetApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_sync_events();
        handle_window_resize(ui);

        #[cfg(windows)]
        if self.pin_mode == PinMode::Desktop && !self.desktop_anchored {
            self.desktop_anchored = desktop_pin::anchor_to_desktop();
        }

        let is_focused = ui.input(|i| i.viewport().focused.unwrap_or(true));
        if is_focused {
            self.manually_minimized = false;
            #[cfg(windows)]
            desktop_pin::reset_manual_minimize();
        }

        #[cfg(windows)]
        if self.pin_mode == PinMode::Desktop && self.was_focused && !is_focused {
            desktop_pin::send_to_bottom();
        }

        self.was_focused = is_focused;

        if self.pin_mode == PinMode::Desktop
            && !self.manually_minimized
            && ui.input(|i| i.viewport().minimized.unwrap_or(false))
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            #[cfg(windows)]
            desktop_pin::anchor_to_desktop();
        }

        let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        let corner_radius = if is_maximized { 0 } else { 8 };

        egui::Frame::default()
            .fill(COLOR_BG)
            .corner_radius(CornerRadius::same(corner_radius))
            .stroke(Stroke::new(1.0, COLOR_BORDER))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

                let header_height = 28.0;
                let (header_rect, header_response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), header_height),
                    Sense::click_and_drag(),
                );

                if header_response.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                if header_response.double_clicked() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                let mut header_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );

                let (dot_rect, _) = header_ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                let dot_color = if self.is_syncing {
                    COLOR_TAG_AMBER_TEXT
                } else if self.sync_status.starts_with("ERR") {
                    COLOR_TAG_RED_TEXT
                } else {
                    Color32::from_rgb(74, 222, 128)
                };
                header_ui.painter().circle_filled(dot_rect.center(), 3.5, dot_color);

                header_ui.label(
                    RichText::new("TASKS")
                        .font(FontId::monospace(13.0))
                        .strong()
                        .color(COLOR_TEXT_PRIMARY),
                );

                header_ui.label(
                    RichText::new(&self.sync_status)
                        .font(FontId::monospace(10.0))
                        .color(COLOR_TEXT_MUTED),
                );

                header_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pixel_icon_btn(ui, "close", CLOSE, 14.0, false, "Close").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    let (max_icon_name, max_icon_bytes, max_tooltip) = if is_maximized {
                        ("copy", COPY, "Restore Down")
                    } else {
                        ("square", SQUARE, "Maximize")
                    };
                    if pixel_icon_btn(ui, max_icon_name, max_icon_bytes, 14.0, false, max_tooltip).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    if pixel_icon_btn(ui, "minus", MINUS, 14.0, false, "Minimize").clicked() {
                        self.manually_minimized = true;
                        #[cfg(windows)]
                        desktop_pin::allow_manual_minimize();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    if pixel_icon_btn(ui, "reload", RELOAD, 14.0, self.is_syncing, "Sync Now").clicked() {
                        self.trigger_sync();
                    }

                    if pixel_icon_btn(
                        ui,
                        "map-pin",
                        MAP_PIN,
                        14.0,
                        self.pin_mode.is_active(),
                        self.pin_mode.tooltip(),
                    )
                    .clicked()
                    {
                        self.pin_mode = self.pin_mode.next();
                        #[cfg(windows)]
                        {
                            if self.pin_mode == PinMode::Desktop {
                                self.desktop_anchored = desktop_pin::anchor_to_desktop();
                            } else {
                                desktop_pin::unanchor_from_desktop();
                                self.desktop_anchored = false;
                            }
                        }
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                            self.pin_mode.window_level(),
                        ));
                    }
                });

                ui.add_space(2.0);

                if self.task_lists.len() > 1 {
                    let current_title = self
                        .task_lists
                        .get(self.selected_list_index)
                        .map(|l| l.title.as_str())
                        .unwrap_or("Select list");

                    let mut changed_list_idx: Option<usize> = None;

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("LIST")
                                .font(FontId::monospace(10.0))
                                .color(COLOR_TEXT_MUTED),
                        );

                        let combo = egui::ComboBox::from_id_salt("single_task_list_dropdown")
                            .selected_text(
                                RichText::new(current_title)
                                    .font(FontId::monospace(11.0))
                                    .color(COLOR_TEXT_PRIMARY),
                            )
                            .width(ui.available_width() - 4.0);

                        combo.show_ui(ui, |ui| {
                            for (idx, list) in self.task_lists.iter().enumerate() {
                                let is_sel = idx == self.selected_list_index;
                                if ui
                                    .selectable_label(
                                        is_sel,
                                        RichText::new(&list.title).font(FontId::monospace(11.0)),
                                    )
                                    .clicked()
                                {
                                    changed_list_idx = Some(idx);
                                }
                            }
                        });
                    });

                    if let Some(idx) = changed_list_idx {
                        self.selected_list_index = idx;
                        self.reload_tasks();
                    }
                    ui.add_space(2.0);
                }

                render_quick_add_bar(self, ui);
                ui.add_space(4.0);

                let max_h = ui.available_height() - 28.0;
                render_tasks_list(self, ui, max_h);

                let pending_count = self.tasks.iter().filter(|t| !t.is_completed).count();
                render_bottom_bar(ui, pending_count);
            });
    }
}

fn render_quick_add_bar(app: &mut TasksWidgetApp, ui: &mut egui::Ui) {
    egui::Frame::default()
        .fill(COLOR_CARD)
        .corner_radius(CornerRadius::same(4))
        .stroke(Stroke::new(1.0, COLOR_BORDER))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_edit = egui::TextEdit::singleline(&mut app.new_task_text)
                    .hint_text("Add task (e.g. Call dentist tomorrow)...")
                    .font(FontId::proportional(13.0))
                    .desired_width(ui.available_width() - 28.0)
                    .frame(egui::Frame::new());

                let response = ui.add(text_edit);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    app.add_task();
                    response.request_focus();
                }

                if pixel_icon_btn(ui, "plus", PLUS, 14.0, false, "Add task").clicked() {
                    app.add_task();
                }
            });
        });
}

fn render_bottom_bar(ui: &mut egui::Ui, pending_count: usize) {
    let (footer_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 16.0),
        Sense::hover(),
    );
    let mut footer_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(footer_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    footer_ui.label(
        RichText::new(format!("{pending_count} pending"))
            .font(FontId::monospace(10.0))
            .color(COLOR_TEXT_MUTED),
    );

    footer_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(
            RichText::new("ENTER TO ADD")
                .font(FontId::monospace(9.0))
                .color(COLOR_TEXT_MUTED),
        );
    });
}

fn render_tasks_list(app: &mut TasksWidgetApp, ui: &mut egui::Ui, max_height: f32) {
    let (pending_tree, completed_tree) =
        build_display_tree(&app.tasks, &app.collapsed_parents);
    let completed_count = completed_tree.len();

    let mut toggled_id: Option<String> = None;
    let mut deleted_id: Option<String> = None;
    let mut start_subtask_for: Option<String> = None;
    let mut subtask_to_add: Option<(String, String)> = None;
    let mut cancel_subtask = false;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(max_height)
        .show(ui, |ui| {
            if pending_tree.is_empty() && completed_tree.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No tasks remaining")
                            .font(FontId::monospace(12.0))
                            .color(COLOR_TEXT_MUTED),
                    );
                });
                return;
            }

            for item in &pending_tree {
                render_task_item(
                    ui,
                    item,
                    &mut app.collapsed_parents,
                    false,
                    &mut toggled_id,
                    &mut deleted_id,
                    &mut start_subtask_for,
                );

                if app.adding_subtask_to.as_deref() == Some(&item.task.id) {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.add(icon("corner-down-right", CORNER_DOWN_RIGHT, 12.0, COLOR_ACCENT));

                        let text_edit = egui::TextEdit::singleline(&mut app.new_subtask_text)
                            .hint_text("Add subtask...")
                            .font(FontId::proportional(12.0))
                            .desired_width((ui.available_width() - 56.0).max(60.0))
                            .frame(egui::Frame::new());

                        let resp = ui.add(text_edit);
                        resp.request_focus();
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            subtask_to_add = Some((item.task.id.clone(), app.new_subtask_text.clone()));
                        }

                        if pixel_icon_btn(ui, "plus", PLUS, 11.0, false, "Add").clicked() {
                            subtask_to_add = Some((item.task.id.clone(), app.new_subtask_text.clone()));
                        }

                        if pixel_icon_btn(ui, "close", CLOSE, 11.0, false, "Cancel").clicked() {
                            cancel_subtask = true;
                        }
                    });
                }

                ui.add_space(4.0);
            }

            if completed_count > 0 {
                ui.add_space(8.0);
                let header_text = format!("COMPLETED ({completed_count})");

                egui::CollapsingHeader::new(
                    RichText::new(header_text)
                        .font(FontId::monospace(10.0))
                        .color(COLOR_TEXT_MUTED),
                )
                .show(ui, |ui| {
                    for item in &completed_tree {
                        render_task_item(
                            ui,
                            item,
                            &mut app.collapsed_parents,
                            true,
                            &mut toggled_id,
                            &mut deleted_id,
                            &mut start_subtask_for,
                        );
                        ui.add_space(4.0);
                    }
                });
            }
        });

    if let Some(parent_id) = start_subtask_for {
        app.adding_subtask_to = Some(parent_id);
        app.new_subtask_text.clear();
    }
    if cancel_subtask {
        app.adding_subtask_to = None;
        app.new_subtask_text.clear();
    }
    if let Some((pid, text)) = subtask_to_add {
        app.add_subtask(&pid, &text);
    }
    if let Some(id) = toggled_id {
        app.toggle_task(&id);
    }
    if let Some(id) = deleted_id {
        app.delete_task(&id);
    }
}

fn spawn_background_sync(
    db: Database,
    sync_trigger_rx: Receiver<()>,
    event_tx: Sender<SyncEvent>,
) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            let _ = event_tx.send(SyncEvent::SyncFailed("Runtime init failed".into()));
            return;
        };

        let mut client: Option<GoogleTasksClient> = None;
        let mut db = db;

        let do_sync = |rt: &tokio::runtime::Runtime, client: &mut Option<GoogleTasksClient>, db: &mut Database| {
            let _ = event_tx.send(SyncEvent::SyncStarted);
            let res = rt.block_on(async {
                if client.is_none() {
                    *client = obtain_authenticated_client().await.ok();
                }
                let Some(c) = client.as_ref() else {
                    return Err("Auth failed".to_string());
                };
                let _ = sync_local_to_db(c, db).await;
                sync_remote_to_db(c, db).await.map_err(|e| e.to_string())
            });

            match res {
                Ok(_) => { let _ = event_tx.send(SyncEvent::SyncCompleted); }
                Err(err) => { let _ = event_tx.send(SyncEvent::SyncFailed(err)); }
            }
        };

        loop {
            do_sync(&rt, &mut client, &mut db);
            match sync_trigger_rx.recv_timeout(Duration::from_secs(60)) {
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn main() -> eframe::Result<()> {
    dotenvy::dotenv().ok();

    let db_path = if std::path::Path::new("task_lists.db").exists() {
        std::path::PathBuf::from("task_lists.db")
    } else if let Ok(exe_dir) = std::env::current_exe().map(|p| p.parent().unwrap_or(&p).to_path_buf()) {
        let candidate = exe_dir.join("task_lists.db");
        if candidate.exists() {
            candidate
        } else {
            exe_dir
                .ancestors()
                .nth(2)
                .map(|p| p.join("task_lists.db"))
                .unwrap_or(candidate)
        }
    } else {
        std::path::PathBuf::from("task_lists.db")
    };

    let db = match Database::new(db_path.to_str().unwrap_or("task_lists.db")) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open task_lists.db: {e}");
            return Ok(());
        }
    };

    let (sync_trigger_tx, sync_trigger_rx) = channel::<()>();
    let (event_tx, event_rx) = channel::<SyncEvent>();

    spawn_background_sync(db.clone(), sync_trigger_rx, event_tx);

    let icon_bytes = include_bytes!("../assets/icon.png");
    let icon_data = image::load_from_memory(icon_bytes).ok().map(|img| {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        }
    });

    let mut viewport_builder = egui::ViewportBuilder::default()
        .with_inner_size([380.0, 520.0])
        .with_min_inner_size([320.0, 360.0])
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(true);

    if let Some(icon) = icon_data {
        viewport_builder = viewport_builder.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport: viewport_builder,
        ..Default::default()
    };

    eframe::run_native(
        "Google Tasks",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(TasksWidgetApp::new(db, sync_trigger_tx, event_rx)))
        }),
    )
}
