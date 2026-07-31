# gtasks_tui

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Language: Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/gtasks_tui.svg)](https://crates.io/crates/gtasks_tui)

`gtasks_tui` is a modern, fast, keyboard-driven terminal dashboard for managing **Google Tasks** on Linux. Built with **Ratatui** and **Crossterm**, it is designed to provide high-performance productivity directly in your terminal.

It follows an **offline-first** architecture: all changes made offline are safely cached in a local SQLite database and synchronized with your Google Tasks account once your internet connection is available.

---

## 🎯 Key Features

*   ⌨️ **Keyboard-Driven Navigation:** Intuitive mappings inspired by Vim for zero-friction navigation.
*   📴 **Offline-First Resilience:** Instant app loading and responsive reads/writes using local SQLite persistence.
*   🧠 **Smart Date Entry (NLP):** Natural language date processing (e.g. typing `"Submit assignment next friday"` creates a task titled `"Submit assignment"` due next Friday).
*   🌲 **Hierarchical Subtask Trees:** Renders subtasks nested under their parents, mirroring the organization of the official Google Tasks clients.
*   🔑 **Secure Keychain Storage:** Protects your OAuth credentials using the system keyring service (e.g. Gnome Keyring, KWallet) via D-Bus APIs.

---

## 🛠️ System Requirements & Build Dependencies

To build `gtasks_tui` from source on Linux, ensure you have the required development headers installed:

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libsecret-1-dev

# Fedora
sudo dnf install @development-tools openssl-devel libsecret-devel

# Arch Linux
sudo pacman -S base-devel openssl libsecret
```

---

## 🚀 Installation & Setup

### 1. Installation
Install the binary directly via Cargo:

```bash
cargo install gtasks_tui
```

### 2. Google OAuth Configuration
In order to authenticate with your Google account, create a `.env` file in the directory from which you run `gtasks_tui` (or load them into your environment variables):

```env
GOOGLE_CLIENT_ID=your_client_id_here
GOOGLE_CLIENT_SECRET=your_client_secret_here
```

To configure OAuth Credentials, visit the [Google Cloud Console](https://console.cloud.google.com/), create a Desktop application project, enable the **Google Tasks API**, and retrieve your OAuth Client ID and Secret.

---

## ⌨️ Comprehensive Keybindings Reference

Interact with the terminal dashboard using the following controls:

### Navigation & Layout Focus
*   `Tab` / `BackTab` - Toggle focus between the **Task Lists sidebar** and the **Tasks list**.
*   `j` / `Down Arrow` - Scroll selection down.
*   `k` / `Up Arrow` - Scroll selection up.
*   `g` - Jump to the top of the active list.
*   `G` - Jump to the bottom of the active list.

### Task List Management (Focused on Lists Sidebar)
*   `L` - Create a new Task List.

### Task Management (Focused on Tasks Pane)
*   `c` - Create a new task in the active list.
*   `e` - Edit the selected task's title, description, and due date.
*   `d` / `Delete` - Delete the selected task (soft-deletes locally, synced remotely).
*   `Space` - Toggle task completion status (`[ ]` ↔ `[x]`).

### Synchronization & System
*   `r` - Force manual delta sync with the Google Tasks server.
*   `q` - Safely save pending items and exit the application.

---

## ⚙️ Configuration & Diagnostics

Logs and temporary trace parameters are written using `tracing`. If you encounter any authentication or runtime bugs, you can run the TUI with elevated logging level:

```bash
RUST_LOG=debug gtasks_tui
```

## 📄 License

This binary is licensed under the **MIT License**.
