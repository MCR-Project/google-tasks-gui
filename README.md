# 🚀 gtasks-gui

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Language: Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/badge/Build-Passing-brightgreen.svg)](https://github.com/MCR-Project/google-tasks-gui)
[![Platform: Linux](https://img.shields.io/badge/Platform-Linux-blue.svg)](https://www.kernel.org/)

> A modern, fast, offline-first **Google Tasks** desktop client & terminal dashboard for Linux built with **Rust**.

---

## 📌 Overview & Highlights

**gTasks** is a high-performance native desktop client for Google Tasks on Linux. It bridges the gap between terminal productivity and desktop UI by providing a fast, offline-first experience with seamless background synchronization.

Whether you prefer a keyboard-driven Terminal User Interface (TUI) or a sleek GTK4 desktop app, **gTasks** keeps your task lists in sync while storing your credentials safely in your OS keyring.

### ✨ Key Features

- ⚡ **Instant Startup & Minimal Footprint:** Written in native Rust with low memory overhead.
- 📴 **Offline-First Storage:** Integrated local SQLite cache so you can read, create, edit, and complete tasks without an active internet connection.
- 🔐 **Secure Keyring Authentication:** Local loopback OAuth 2.0 PKCE flow that stores tokens safely in the Linux System Keyring (`Secret Service` API / KWallet).
- 🖥️ **Dual Interface:**
  - **TUI:** Terminal User Interface built with [Ratatui](https://github.com/ratatui/ratatui) & [Crossterm](https://github.com/crossterm-rs/crossterm).
  - **GUI:** Desktop interface built with GTK4 / Libadwaita via [Relm4](https://relm4.org/).
- 🗓️ **Natural Language Date Parsing:** Smart date recognition (e.g. `"Buy milk today"`, `"Submit report tomorrow"`, `"Team sync next monday"`).
- 🌲 **Hierarchical Subtask Ordering:** Supports nested tasks and subtask parent-child relationships.
- 🔄 **Bidirectional Background Sync:** Automatic delta syncing, conflict resolution, offline change queuing, and silent OAuth token refreshes.

---

## 🏗️ Workspace Architecture

```text
               ┌────────────────────────────────────────────────────────┐
               │          Terminal UI           │      Desktop GUI      │
               │      (Ratatui / Crossterm)     │   (Relm4 / GTK4)      │
               └───────────────────┬───────────────────┬────────────────┘
                                   │                   │
                                   ▼                   ▼
               ┌────────────────────────────────────────────────────────┐
               │                   gtasks_core Engine                   │
               │  • Tokio Async Runtime      • OAuth 2.0 PKCE Handler   │
               │  • Background Sync Engine   • OS Keyring Credentials   │
               │  • NLP Date Parsing         • Task Hierarchy Processor │
               └──────────────┬────────────────────────┬────────────────┘
                              │                        │
               ┌──────────────▼──────┐          ┌──────▼────────────────┐
               │ Local SQLite Cache  │          │ Google Tasks REST API │
               │  (Fast offline read)│          │ (Background HTTP Sync)│
               └─────────────────────┘          └───────────────────────┘
```

The repository is structured as a Rust Cargo Workspace:
- [`crates/gtasks_core`](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core) - Core business logic, OAuth 2.0 PKCE flow, SQLite database persistence, background sync worker, and natural language date parser.
- [`crates/gtasks_tui`](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui) - Terminal dashboard application powered by Ratatui & Crossterm.
- [`crates/gtasks_gui`](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui) - Native Linux desktop GUI built with Relm4 (GTK4 / Libadwaita).

---

## 🛠️ Prerequisites & System Requirements

To build **gTasks** on Linux, ensure you have the Rust toolchain installed alongside native development headers:

### 1. Install System Dependencies

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libsecret-1-dev libgtk-4-dev libadwaita-1-dev

# Fedora
sudo dnf install @development-tools openssl-devel libsecret-devel gtk4-devel libadwaita-devel

# Arch Linux / Manjaro
sudo pacman -S base-devel openssl libsecret gtk4 libadwaita
```

### 2. Configure Google API OAuth Credentials

Create a `.env` file in the root directory (or copy from template) containing your Google Cloud OAuth Client ID and Secret:

```env
GOOGLE_CLIENT_ID=your_client_id_here
GOOGLE_CLIENT_SECRET=your_client_secret_here
```

---

## 🚀 Quick Start

### Building & Running

```bash
# Clone the repository
git clone https://github.com/MCR-Project/google-tasks-gui.git
cd google-tasks-gui

# Run the Terminal User Interface (TUI)
cargo run -p gtasks_tui

# Run the Desktop GUI (GTK4 / Relm4)
cargo run -p gtasks_gui
```

### Running Tests

```bash
cargo test
```

---

## ⌨️ TUI Keybindings

When running `gtasks_tui`, use the following keyboard controls:

| Key | Action |
| --- | --- |
| `Tab` | Switch focus between Task Lists sidebar and Tasks view |
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `g` | Jump to top of list |
| `G` | Jump to bottom of list |
| `Space` | Toggle task completion status (`[ ]` / `[x]`) |
| `c` | Create new task in active list |
| `L` | Create new task list (when focused on Task Lists pane) |
| `e` | Edit selected task details (Title, Notes, Due Date) |
| `d` / `Delete` | Delete selected task |
| `r` | Trigger manual sync with Google Tasks API |
| `q` | Quit application |

---

## 🧠 Natural Language Task Entry

When creating or editing tasks, natural language expressions in task titles are automatically parsed into structured due dates:

| Input Text | Resulting Task Title | Extracted Due Date |
| --- | --- | --- |
| `Buy groceries today` | `Buy groceries` | Today |
| `Finish quarterly report tomorrow` | `Finish quarterly report` | Tomorrow |
| `Sync with team next monday` | `Sync with team` | Next Monday |

---

## 🗺️ Roadmap & Project Status

- [x] **Phase 0: Workspace Architecture**
  - Modular Cargo workspace (`gtasks_core`, `gtasks_tui`, `gtasks_gui`).
- [x] **Phase 1: Secure Core & Engine**
  - OAuth 2.0 PKCE authentication flow with local loopback listener.
  - Linux System Keyring integration (`Secret Service` / KWallet).
  - SQLite local database cache (`task_lists.db`).
  - Google Tasks REST API client integration.
  - Natural Language Date Parsing (NLP).
- [x] **Phase 2: User Interfaces**
  - Interactive Terminal User Interface (`gtasks_tui`).
  - Native Linux GTK4 Desktop GUI (`gtasks_gui`).
- [x] **Phase 3: Sync & Offline Resilience**
  - Offline edit queuing with `dirty_bit` flag tracking.
  - Background delta sync engine.
  - Soft-deletion queue & sync.
- [ ] **Phase 4: Packaging & Distribution**
  - Flatpak & AUR packaging.
  - Desktop entry & icon integration.

---

## 🤝 Contributing

Contributions are very welcome! If you find a bug or have a feature suggestion, please feel free to open an issue or submit a pull request.

1. Fork the Repository
2. Create your Feature Branch (`git checkout -b feature/amazing-feature`)
3. Commit your Changes (`git commit -m 'Add some amazing-feature'`)
4. Verify tests pass (`cargo test`)
5. Push to the Branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

---

## 🔒 Privacy Policy

gTasks is a local-first application and prioritizes your data privacy. Google Tasks data and OAuth tokens are strictly stored locally on your device or in your OS Keyring and are never transmitted to any third-party servers. For details, see the complete [Privacy Policy](file:///home/alex_insc/project/gtasks-tui/PRIVACY.md).

---

## 📄 License

This project is licensed under the **MIT License** - see the [LICENSE](file:///home/alex_insc/project/gtasks-tui/LICENSE) file for details.
