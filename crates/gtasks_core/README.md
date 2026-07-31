# gtasks_core

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Language: Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/gtasks_core.svg)](https://crates.io/crates/gtasks_core)

`gtasks_core` is the foundational engine and synchronization library behind **gtasks**, a modern Google Tasks client ecosystem for Linux. It provides low-level integrations for OAuth 2.0 PKCE authentication, SQLite database operations, real-time sync conflict handling, and natural language date parsing.

This library can be used independently to build custom clients, automation daemons, or scripting utilities interacting with the Google Tasks API.

---

## 🚀 Key Features

*   🔄 **Delta Synchronization Engine:** Efficiently syncs remote changes using `updated` timestamps. Tracks local modifications with a dirty-bit state to push changes back upstream.
*   📴 **Offline-First Storage:** Integrated thread-safe SQLite database wrapper (`rusqlite`) that manages local schemas, transactions, and soft-deletions.
*   🔑 **Secure Credentials Flow:** Implements standard OAuth 2.0 Authorization Code Flow with PKCE. Saves encrypted refresh tokens securely using the OS Keyring (`keyring` crate targeting D-Bus Secret Service, KWallet, etc.).
*   🌲 **Hierarchy Processing:** Processes list responses into parent-child subtask trees, preserving visual ordering.
*   🧠 **Natural Language Parsing:** Parses smart terms (like `"tomorrow"`, `"next monday"`) directly from string inputs to set appropriate task due dates.

---

## 🛠️ Usage & Examples

### 1. Connecting and Authenticating
The engine handles the login state automatically, requesting user authentication via browser loopback server if no refresh token is stored in the system keyring.

```rust
use gtasks_core::obtain_authenticated_client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Retrieves client using local OS Keyring or spawns browser flow
    let mut client = obtain_authenticated_client().await?;
    
    // Fetch remote task lists
    let lists = client.get_task_lists().await?;
    for list in lists {
        println!("List: {} ({})", list.title, list.id);
    }
    
    Ok(())
}
```

### 2. Initializing the Local SQLite Database
`gtasks_core` packages a database manager that automatically sets up and manages tables for lists and tasks:

```rust
use gtasks_core::Database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new("task_lists.db")?;
    
    // Retrieve cached tasks for a list
    let cached_tasks = db.get_tasks("some-list-id")?;
    for task in cached_tasks {
        println!("Task: {} [Completed: {}]", task.title, task.is_completed);
    }
    
    Ok(())
}
```

### 3. Parsing Natural Language Dates
You can extract dates from user input:

```rust
use gtasks_core::util::parse_nlp_task;

fn main() {
    let raw_input = "Write Rust code tomorrow";
    let (cleaned_title, parsed_due_date) = parse_nlp_task(raw_input);
    
    assert_eq!(cleaned_title, "Write Rust code");
    assert!(parsed_due_date.is_some());
    println!("Due date parsed: {:?}", parsed_due_date);
}
```

---

## 🧱 Architecture Details

```text
               ┌───────────────────────────────────────┐
               │         Client Application            │
               └───────────┬───────────────────┬───────┘
                           │                   │
                           ▼                   ▼
               ┌───────────────────────────────────────┐
               │          gtasks_core API              │
               └───────────┬───────────────────┬───────┘
                           │                   │
                           ▼                   ▼
               ┌───────────────────────┐   ┌───────────────────────┐
               │    Local DB (SQLite)  │   │ Google REST API Sync  │
               └───────────────────────┘   └───────────────────────┘
```

The core is structured as:
*   **`api`**: Wrappers around the Google Tasks REST endpoints.
*   **`auth`**: PKCE flow logic and OS keychain integration.
*   **`db`**: Thread-safe database transactions and updates.
*   **`sync`**: Synchronization worker coordination.
*   **`util`**: Date extraction, task trees, and helpers.

---

## 🛠️ System Prerequisites

Building the crate requires standard SSL, Secret Service, and GCC headers on your system:

```bash
# Ubuntu / Debian
sudo apt install build-essential pkg-config libssl-dev libsecret-1-dev

# Fedora
sudo dnf install @development-tools openssl-devel libsecret-devel

# Arch Linux
sudo pacman -S base-devel openssl libsecret
```

---

## 🤝 Contributing

Contributions are welcome! Submit a PR or open issues for feature requests.

## 📄 License

This library is licensed under the **MIT License**.
