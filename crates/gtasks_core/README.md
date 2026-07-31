# gtasks_core

[![Crates.io](https://img.shields.io/crates/v/gtasks_core.svg?style=flat-square)](https://crates.io/crates/gtasks_core)
[![docs.rs](https://img.shields.io/docsrs/gtasks_core?style=flat-square)](https://docs.rs/gtasks_core)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/MCR-Project/google-tasks-gui/ci.yml?branch=main&style=flat-square)](https://github.com/MCR-Project/google-tasks-gui)

`gtasks_core` is an offline-first, thread-safe Rust SDK and engine for the **Google Tasks API**. 

Designed as the core foundation for terminal (TUI) and graphical (GUI) task managers, `gtasks_core` provides zero-hassle OAuth 2.0 PKCE authentication, automatic OS keyring credential persistence, thread-safe SQLite local caching, background delta synchronization, and natural-language date parsing.

---

## ✨ Features

* 🔐 **Automatic OAuth 2.0 + PKCE:** Handles the browser authentication handshake and securely persists refresh tokens in system secret stores (D-Bus Secret Service, KWallet, macOS Keychain).
* 🔄 **Bidirectional Delta Synchronization:** Periodically synchronizes remote modifications using `updatedMin` RFC3339 timestamps while maintaining local dirty states for offline operations.
* 📴 **Offline-First SQLite Cache:** High-performance, thread-safe SQLite persistence layer powered by `rusqlite` for local read/write performance.
* 🌲 **Subtask Tree Ordering:** Processes flat API responses into hierarchical parent-child task structures while maintaining custom visual ordering.
* 🧠 **Natural Language Processing (NLP):** Built-in date parser extracting temporal expressions like `"today"`, `"tomorrow"`, or `"next monday"` directly from task titles.
* ⚡ **Actor-Based Background Sync:** Non-blocking async `SyncManager` actor for background sync polling and window focus triggers.

---

## 📦 Installation

Add `gtasks_core` to your `Cargo.toml`:

```toml
[dependencies]
gtasks_core = "0.1.1"
tokio = { version = "1.0", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
```

### System Prerequisites

To build system keyring bindings (`keyring`) and OpenSSL dependencies on Linux, install the required development headers:

```bash
# Ubuntu / Debian
sudo apt install build-essential pkg-config libssl-dev libsecret-1-dev

# Fedora
sudo dnf install @development-tools openssl-devel libsecret-devel

# Arch Linux
sudo pacman -S base-devel openssl libsecret
```

---

## 🚀 Quickstart

### 1. Authenticate & Perform API Operations

`obtain_authenticated_client()` automatically retrieves saved credentials from the system keyring, or initiates a browser OAuth PKCE authorization loop if no valid token exists.

```rust
use gtasks_core::obtain_authenticated_client;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Authenticate (Keyring lookup -> Browser PKCE fallback)
    let mut client = obtain_authenticated_client().await?;

    // 2. Fetch remote task lists
    let lists = client.get_task_lists().await?;
    println!("Found {} task lists:", lists.len());
    
    for list in &lists {
        println!(" - {} (ID: {})", list.title, list.id);
    }

    // 3. Create a new task list
    let task_list = client
        .create_task_list("Shopping List")
        .await?;
    println!("Created task list: {}", task_list.title);

    Ok(())
}
```

### 2. Offline-First SQLite Synchronization

Use `Database` for local persistence and `sync_remote_to_db` for bidirectional synchronization:

```rust
use gtasks_core::{obtain_authenticated_client, Database, sync_remote_to_db};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut client = obtain_authenticated_client().await?;
    let mut db = Database::new("tasks_cache.db")?;

    // Perform bidirectional sync: Local dirty tasks -> Google API -> SQLite
    sync_remote_to_db(&mut client, &mut db).await?;

    // Query cached tasks offline
    let tasks = db.get_all_tasks()?;
    for task in tasks {
        let status = if task.is_completed { "✓" } else { " " };
        println!("[{}] {}", status, task.title.unwrap_or_default());
    }

    Ok(())
}
```

---

## 🏗️ Architecture Overview

```text
               ┌──────────────────────────────────────────────┐
               │    Client App (TUI / GUI / CLI Script)       │
               └──────────────┬────────────────┬──────────────┘
                              │                │
            Local Queries &   │                │ Background Events
            Offline Writes    ▼                ▼
               ┌────────────────────┐    ┌────────────────────┐
               │  Database (SQLite) │    │    SyncManager     │
               └─────────▲──────────┘    └─────────┬──────────┘
                         │                         │
                         └──────────┬──────────────┘
                                    │ Network Sync
                                    ▼
               ┌──────────────────────────────────────────────┐
               │         GoogleTasksClient (REST API)         │
               └────────────────────┬─────────────────────────┘
                                    │ HTTPS (Bearer Auth)
                                    ▼
               ┌──────────────────────────────────────────────┐
               │              Google Tasks API                │
               └──────────────────────────────────────────────┘
```

The library is organized into specialized modules:

| Module | Description |
| :--- | :--- |
| [`api`](https://docs.rs/gtasks_core/latest/gtasks_core/api/index.html) | Direct Google Tasks REST API client with exponential backoff and retry handlers. |
| [`auth`](https://docs.rs/gtasks_core/latest/gtasks_core/auth/index.html) | OAuth 2.0 PKCE challenge generator, token refresh, and OS keyring persistence. |
| [`db`](https://docs.rs/gtasks_core/latest/gtasks_core/db/index.html) | Thread-safe SQLite schema, CRUD operations, transactions, and soft-delete queues. |
| [`sync`](https://docs.rs/gtasks_core/latest/gtasks_core/sync/index.html) | MPSC actor managing active/idle sync interval loops and window state updates. |
| [`util`](https://docs.rs/gtasks_core/latest/gtasks_core/util/index.html) | Natural language date parsing and hierarchical task tree layout algorithms. |

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome! Feel free to check the [issues page](https://github.com/MCR-Project/google-tasks-gui/issues).

## 📄 License

Distributed under the **MIT License**. See `LICENSE.md` for details.
