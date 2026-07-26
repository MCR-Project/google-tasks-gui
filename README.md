# 🚀 gTasks (`gtasks-ru`)

> A modern, fast, offline-first Google Tasks desktop client for Linux built with **Rust**.

---

## 📌 Overview & Vision

While Google Tasks is widely used across mobile and web, Linux lacks a modern, high-performance native desktop client. **gTasks** bridges this gap by providing a native Linux desktop app build on rust, works completely offline, and syncs seamlessly with Google's servers in the background.

### Key Goals

* ⚡ **Instant Startup & Low Footprint:** Native performance with minimal RAM usage.
* 📴 **Offline-First:** Local SQLite cache so you can view, create, and complete tasks without an active internet connection.
* 🔐 **Secure Auth:** Local loopback OAuth 2.0 PKCE authentication storing credentials in the Linux system keyring (`Secret Service` API / KWallet).
* 🐧 **Native Desktop Integration:** Global system tray support, dark mode sync, and native Linux desktop notifications.

---

## 🏗️ Architecture & Data Flow

```text
               ┌─────────────────────────────────────────────────┐
               │             Desktop Application                 │
               │   (Tauri 2.0 / GTK4 Libadwaita Interface)       │
               └────────────────────────┬────────────────────────┘
                                        │
                                        ▼
               ┌─────────────────────────────────────────────────┐
               │              Rust Engine Core                   │
               │  • Tokio Async Runtime   • OAuth PKCE Handler   │
               │  • Background Sync Loop  • Local Keyring Vault  │
               └──────────────┬──────────────────┬───────────────┘
                              │                  │
               ┌──────────────▼──────┐    ┌──────▼────────────────┐
               │ Local SQLite Cache  │    │ Google Tasks REST API │
               │  (Fast offline read)│    │ (Background sync HTTP)│
               └─────────────────────┘    └───────────────────────┘

```

---

## 🗺️ Project Roadmap & Status

### Phase 0: Architecture & Tooling

* [x] **Tech Stack Selection:** Rust + Tokio + Reqwest backend.
* [x] **Local Storage Design:** SQLite via `sqlx` / `rusqlite` for offline-first data persistence.
* [x] **Authentication Flow:** OAuth 2.0 Authorization Code Flow with PKCE.
* [x] **UI Framework Target:** Tauri 2.0 (HTML/Tailwind UI) or Relm4 (Native GTK4).

---

### Phase 1: Headless Core & CLI Proof of Concept *(Current Focus)*

> **Goal:** Complete a working terminal/CLI tool in Rust before touching any GUI code.

* [x] **Local Keyring Setup:** Integrate `keyring` crate to read/write tokens safely to Linux Secret Service.
* [x] **OAuth 2.0 PKCE Loopback:**
* [x] Spin up local TCP listener on `127.0.0.1:8080`.
* [x] Launch system browser (`open` crate) to Google Login page.
* [x] Receive redirect authorization code and exchange for `access_token` & `refresh_token`.


* [x] **Google API Client:**
* [x] Fetch user task lists (`GET /tasks/v1/users/@me/lists`).
* [x] Save user task lists in a sqlite database
* [x] Fetch tasks within a list (`GET /tasks/v1/lists/{list_id}/tasks`).
* [x] Save users tasks within a list in the sqlite database
* [x] Create a new task (`POST /tasks/v1/lists/{list_id}/tasks`).
* [x] Toggle task completion (`PATCH /tasks/v1/lists/{list_id}/tasks/{task_id}`).
* [x] Delete a task (`DELETE /tasks/v1/lists/{list_id}/tasks/{task_id}`).
* [x] Update tasks

* [x] **Local SQLite Schema:**
* [x] Create `task_lists` table (`id`, `title`, `updated_at`, `synced`).
* [x] Create `tasks` table (`id`, `list_id`, `title`, `notes`, `due_date`, `status`, `dirty_bit`).



---

### Phase 2: GUI Frontend & State Management

> **Goal:** Connect the Rust backend logic to a modern desktop window.

* [ ] **Window Shell Setup:** Initialize application layout with GTK4 / Libadwaita styling.
* [ ] **Task List Sidebar:** Render task lists dynamically from the local SQLite cache.
* [ ] **Task View Grid:** Display tasks, subtasks, due dates, and completion checkboxes.
* [ ] **Keyboard Shortcuts:** Add `Ctrl+N` (New Task), `Ctrl+R` (Sync), and `Escape` (Close/Minimize).
* [ ] **Dark / Light Theme Auto-Detection:** Match system desktop theme automatically.

---

### Phase 3: Background Sync Engine

> **Goal:** Handle network interruptions and background bidirection sync safely.

* [ ] **Offline Queueing:** Mark tasks modified offline with a `dirty` flag in SQLite.
* [ ] **Background Sync Worker:** Tokio task running every 5 minutes (or on manual trigger) to push `dirty` tasks and fetch server updates.
* [ ] **Conflict Resolution:** Simple "Last Write Wins" timestamp comparison strategy.
* [x] **Auto-Refresh Tokens:** Silent token refresh using saved `refresh_token` when HTTP 401 occurs.

---

### Phase 4: Linux Desktop Polish

> **Goal:** Make the app feel like a built-in OS utility.

* [ ] **System Tray Integration:** Minimize to tray icon with a quick "Add Task" popup option.
* [ ] **Desktop Notifications:** Send native Linux desktop notifications when a task due date arrives using `notify-rust`.
* [ ] **Global Hotkey:** Configurable shortcut (e.g., `Super+Shift+T`) to launch or focus the window.

---

### Phase 5: Packaging & Distribution

> **Goal:** Publish to package managers worldwide.

* [ ] **Flatpak Build:** Package for Flathub deployment.
* [ ] **Arch Linux AUR:** Create `PKGBUILD` script for Arch / Manjaro users.
* [ ] **Debian / Ubuntu Package:** Build `.deb` binary package using `cargo-deb`.
* [ ] **Internationalization (i18n):** Extract all UI strings into gettext / fluent files for global translations.

---

## 🛠️ Required Dependencies (`Cargo.toml`)

```toml
[dependencies]
# Async Engine & HTTP
tokio = { version = "1.38", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Local Storage & System Auth
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-native-tls"] }
keyring = "2.1"
open = "5.1"
url = "2.5"

# Linux Desktop Integration
notify-rust = "4.10"

```

---

## 💻 Developer Setup

### Prerequisites

Make sure you have basic C build tools and Linux header libraries installed:

```bash
# Ubuntu / Debian
sudo apt install build-essential pkg-config libssl-dev libsecret-1-dev

# Fedora
sudo dnf install @development-tools openssl-devel libsecret-devel

# Arch Linux
sudo pacman -S base-devel openssl libsecret

```

### Running Local Builds

```bash
# Clone the repository
git clone https://github.com/your-username/taska.git
cd taska

# Run CLI proof-of-concept
cargo run --bin taska-cli

```

---

## 📄 License

Distributed under the **MIT License**. See `LICENSE` for more information.

---