# 🔍 GTasks Workspace — Full Architectural & Code Quality Audit

> **Scope**: `gtasks_core` (7 files, 1,242 LoC) · `gtasks_gui` (1 file, 1,284 LoC) · `gtasks_tui` (4 files, 1,043 LoC)
> **Total**: 3,735 lines of Rust across 12 source files  
> **Audit Date**: 2026-07-30

---

## 📊 Workspace Scorecard & Ratings

| Crate | Architecture | Cleanliness | Concurrency | DB & Sync Safety | **Overall** |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `gtasks_core` | **8/10** | **7/10** | **6/10** | **9/10** | **7.5/10** |
| `gtasks_gui`  | **6/10** | **6/10** | **7/10** | **8/10** | **6.8/10** |
| `gtasks_tui`  | **8/10** | **7/10** | **6/10** | **8/10** | **7.3/10** |

**Workspace Overall: 7.2/10** — A solid, functional codebase with clear intent and reasonable separation. The main deductions come from DRY violations in the DB layer, a monolithic GUI file, blocking I/O on async threads, and duplicated NLP logic across frontends.

---

## 1. Architecture — Detailed Breakdown

### `gtasks_core` — **8/10**

**Strengths:**
- Clean module hierarchy: `api/` (models + HTTP), `auth/` (OAuth PKCE + keyring), `db/` (SQLite), `sync.rs` (actor), [lib.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs) (orchestration).
- Zero UI dependencies in `Cargo.toml` — no `ratatui`, `crossterm`, `relm4`, or GTK types.
- `SyncManager` uses a clean actor pattern with `mpsc` channels ([sync.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs)).
- `lib.rs` exports a clean public API surface: `obtain_authenticated_client`, `sync_remote_to_db_delta`, `sync_local_to_db`.

**Deductions:**
- **UI concept leak** (-1): [SyncCommand::SetWindowActive(bool)](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L12) exposes a "window focus" concept in core. A UI-agnostic name like `SetAppActive` or `SetForeground` would preserve purity.
- **Model dual-purpose** (-1): [TaskLocal](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L32-L45) lives in `api/mod.rs` but carries DB-specific fields (`is_dirty`, `is_deleted`). It's a hybrid API/DB model — not a clean domain separation. Ideally, `api` would own wire types and `db` would own persistence types with a mapping layer.

### `gtasks_gui` — **6/10**

**Strengths:**
- Properly delegates all DB access to `gtasks_core::Database` and sync to `SyncManager`.
- Uses `relm4::spawn` for async work, keeping the GTK main loop responsive.
- Clean [Relm4 factory pattern](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L27-L52) for list and task rows.

**Deductions:**
- **Single 1,284-line file** (-2): [main.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs) contains models, UI view, factory components, NLP parsing, task ordering logic, and all update handlers. No modularization at all.
- **Duplicated business logic** (-1): [parse_nlp_task](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L138-L226) (89 lines) and [order_tasks_hierarchically](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L228-L264) are copy-pasted from the TUI. These belong in `gtasks_core`.
- **Direct API calls** (-1): [DeleteList](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L932-L938) spawns a direct `obtain_authenticated_client().await` + `delete_task_list()` call, bypassing the `SyncManager` and creating a second auth path.

### `gtasks_tui` — **8/10**

**Strengths:**
- Clean 3-file module split: [mod.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/mod.rs) (state + logic), [draw.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/draw.rs) (rendering), [run.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/run.rs) (event loop + async).
- Good `TerminalGuard` RAII pattern for terminal cleanup ([run.rs:16-24](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/run.rs#L16-L24)).
- Background sync channel pattern with `BackgroundAction` and result feedback ([run.rs:26-92](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/run.rs#L26-L92)).
- Delegates all DB operations to `gtasks_core::Database`.

**Deductions:**
- **No `SyncManager` usage** (-1): Unlike the GUI, the TUI creates its own ad-hoc background sync via raw `tokio::spawn` instead of using the shared `SyncManager` from core.
- **Duplicated NLP logic** (-1): [parse_nlp_task](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/mod.rs#L51-L139) is an identical copy of the GUI version.

---

## 2. Code Cleanliness & Idiomatic Rust — Detailed Breakdown

### Quantitative Findings

| Metric | `gtasks_core` | `gtasks_gui` | `gtasks_tui` | Notes |
|:---|:---:|:---:|:---:|:---|
| `.unwrap()` calls | **0** prod / 0 test | **2** (L221, L807) | **2** (L134, L391) | All 4 are `and_hms_opt(0,0,0).unwrap()` — safe but not idiomatic |
| `.expect()` calls | **0** prod / **11** test | **0** | **0** | All in `#[cfg(test)]` — acceptable |
| `panic!()` | **0** | **0** | **0** | ✅ |
| `println!()` | **0** | **0** | **3** (L11, L23, L32) | TUI startup/shutdown messages bypass `tracing` |
| `eprintln!()` | **0** | **0** | **0** | ✅ |
| `unsafe` blocks | **0** | **0** | **0** | ✅ |
| `lock().unwrap()` | **0** | **0** | **0** | ✅ `lock_conn()` maps poison errors properly |
| `let _ = ...` (silent failures) | **~8** | **~5** | **~6** | Silent `Result` drops across sync/channel sends |
| Tests | **3** functions | **3** functions | **3** functions | Reasonable for the codebase size |

### DRY Violations (Critical)

| Violation | Location | Lines Duplicated |
|:---|:---|:---:|
| **Row-to-TaskLocal mapping** | [db/mod.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs) L173-215, L229-271, L286-328, L400-442 | **~160 lines** (4× copy) |
| **`parse_nlp_task()`** | [gui/main.rs:138-226](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L138-L226) ↔ [tui/ui/mod.rs:51-139](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/mod.rs#L51-L139) | **~178 lines** (2× copy) |
| **`order_tasks_hierarchically()`** | [gui/main.rs:228-264](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L228-L264) | Only in GUI; TUI lacks it |
| **TaskLocal construction** in toggle handlers | [gui/main.rs:1130-1146](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L1130-L1146), [1204-1216](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L1204-L1216), [826-850](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L826-L850) | **~60 lines** (3× near-copy) |

### Error Handling Style

- **Good**: Production code uses `Result` + `?` extensively. No panicking unwraps in prod paths.
- **Concerning**: Heavy use of `let _ = ...` to silently discard `Result`s:
  - [lib.rs:63](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L63), [119](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L119), [141](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L141) — DB purge failures silently ignored (could cause sync loops)
  - [sync.rs:37](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L37), [41](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L41), [63](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L63), [65](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L65) — Channel send errors silently dropped
- **Missing**: No custom error type (`thiserror`). Auth uses string-coerced errors like `"OAuth State mismatch".into()`.

---

## 3. Async & Concurrency — Detailed Breakdown

### `gtasks_core` — **6/10**

| Aspect | Status | Details |
|:---|:---:|:---|
| API calls async | ✅ | `reqwest` used throughout `api/mod.rs` |
| DB calls blocking | ⚠️ | `rusqlite` is synchronous. All DB ops in async fns block the tokio worker thread |
| Token refresh retry | ✅ | [execute_with_retry](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L267-L286) handles 401 transparently |
| `lock().unwrap()` | ✅ | [lock_conn()](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L12-L18) properly maps poison errors |
| Thread safety | ✅ | `Arc<Mutex<Connection>>` correctly protects DB |

> [!WARNING]
> **Blocking DB on async runtime**: [sync.rs:101](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L101) calls `Database::new()` (blocking file I/O) inside a tokio-spawned task. [lib.rs:43](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L43), [69](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L69) call `db.save_tasks()` / `db.save_task_lists()` blocking inline in async functions. All should use `tokio::task::spawn_blocking`.

### `gtasks_gui` — **7/10**

- Uses `relm4::spawn` for async work, properly separated from the GTK main loop.
- `SyncManager` runs in a tokio task — good isolation.
- DB reads in `init()` and `update()` are synchronous on the Relm4 component thread. For small DBs this is fine; for scale, it could cause UI jank.

### `gtasks_tui` — **6/10**

- Background channel worker ([run.rs:65-92](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/run.rs#L65-L92)) is a good pattern.
- However, the initial sync in [main.rs:24-27](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/main.rs#L24-L27) silently discards sync errors with `let _ =`.
- The `run()` function's event loop polls with `Duration::from_millis(100)`, which is appropriate for TUI responsiveness.

---

## 4. Database Integrity & Sync Safety — Detailed Breakdown

### `gtasks_core` — **9/10**

| Check | Status | Evidence |
|:---|:---:|:---|
| `PRAGMA foreign_keys = ON` | ✅ | [db/mod.rs:22](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L22) |
| FK constraint defined | ✅ | [db/mod.rs:59](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L59) `FOREIGN KEY(list_id) REFERENCES task_lists(id)` |
| FK constraint tested | ✅ | [test_foreign_keys_enabled](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L487-L504) |
| Transactions with `tx.commit()?` | ✅ | 5 transactional methods ([db/mod.rs:80](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L80), [127](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L127), [143](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L143), [380](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L380), [392](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L392)) |
| Dirty-flag discipline | ✅ | `WHERE excluded.dirty = 1 OR (tasks.dirty = 0 AND ...)` conflict resolution at [db/mod.rs:363](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L363) |
| Dirty-flag test | ✅ | [test_local_dirty_task_not_overwritten_by_remote](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L507-L582) |
| `nextPageToken` pagination | ✅ | Loop in [get_task_lists](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L75-L96) and [get_tasks](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L146-L174) |
| HTTP PATCH partial update | ✅ | [update_task](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L203-L254) builds `serde_json::Map` with only `Some` fields |
| `showDeleted=true` | ✅ | [api/mod.rs:152](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L152) |
| Deleted task handling | ✅ | [lib.rs:62](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L62) purges deleted tasks from local DB |
| Schema migration safety | ⚠️ | [db/mod.rs:41-44](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L41-L44) uses `let _ = conn.execute("ALTER TABLE...")` — silently ignores errors via intent, but fragile |

**Deduction (-1)**: Schema migration via silent `ALTER TABLE` failures is a hack. Use `PRAGMA table_info` or a migration framework.

### GUI & TUI DB Safety — **8/10**

- Both frontends exclusively delegate to `gtasks_core::Database` — no raw SQL in frontend code.
- GUI properly uses `db.save_tasks()`, `db.mark_task_deleted()`, `db.save_task_lists()` for all mutations.
- Minor concern: GUI [ToggleActiveTask](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L1130-L1146) constructs `TaskLocal` with `due: None`, which could overwrite an existing due date when saving back. This is a **data-loss risk**.

---

## 🌟 Key Strengths

1. **Zero `unsafe`, zero `panic!()`, zero `lock().unwrap()` in production code** — a rare achievement in a Rust project this size.

2. **Robust dirty-flag sync discipline** — The `ON CONFLICT` clause at [db/mod.rs:363](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L363) correctly prevents remote sync from overwriting unsaved local edits. This is tested and the test is well-written.

3. **Proper OAuth PKCE flow** — [auth/mod.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/auth/mod.rs) implements PKCE with state validation, compile-time or runtime credential resolution, and automatic token refresh via [execute_with_retry](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/api/mod.rs#L267-L286).

4. **Full pagination support** — Both `get_task_lists()` and `get_tasks()` loop on `nextPageToken`, handling all pages correctly.

5. **Clean SyncManager actor** — [sync.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs) implements a well-structured actor with adaptive polling intervals (30s active / 180s idle), immediate sync on focus, and clean command/event separation.

6. **Good TUI module decomposition** — `mod.rs` (state), `draw.rs` (rendering), `run.rs` (event loop) is a clean, standard TUI architecture pattern.

7. **Well-chosen `tracing` adoption** — Core and GUI use `tracing::info!` / `tracing::error!` consistently for structured logging (aside from 3 `println!` in TUI startup).

---

## ⚠️ Identified Code Smells & Tech Debt

### 🔴 High Priority

| # | Issue | Location | Impact |
|:--|:------|:---------|:-------|
| 1 | **4× copy-pasted row-to-TaskLocal mapping** (~160 LoC) | [db/mod.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs) L173-215, L229-271, L286-328, L400-442 | Maintenance nightmare; fixing a bug requires 4 edits |
| 2 | **Duplicated `parse_nlp_task()` across GUI and TUI** (~178 LoC total) | [gui/main.rs:138](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L138) ↔ [tui/mod.rs:51](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/mod.rs#L51) | Feature parity drift; belongs in core |
| 3 | **Blocking `rusqlite` calls on async tokio threads** | [sync.rs:101](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/sync.rs#L101), [lib.rs:43](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L43), [69](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L69) | Can stall the entire tokio runtime under load |
| 4 | **1,284-line monolithic GUI file** | [gui/main.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs) | Unmaintainable; need modular split |

### 🟡 Medium Priority

| # | Issue | Location | Impact |
|:--|:------|:---------|:-------|
| 5 | **Silent `let _ = ...` on critical DB operations** | [lib.rs:63](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L63), [119](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L119), [141](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/lib.rs#L141) | Failed purges can cause infinite re-creation sync loops |
| 6 | **GUI `ToggleActiveTask` sets `due: None`** | [gui/main.rs:1136](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L1136) | **Data loss**: toggling completion wipes the due date |
| 7 | **No custom error type** — uses `Box<dyn Error + Send + Sync>` everywhere | All crates | Lose type-safe error discrimination for callers |
| 8 | **GUI bypasses SyncManager for list deletion** | [gui/main.rs:932-938](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L932-L938) | Creates a parallel auth/API path, potential race condition |
| 9 | **TUI uses `println!()` instead of `tracing`** | [tui/main.rs:11,23,32](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/main.rs#L11) | Logging inconsistency |

### 🟢 Low Priority

| # | Issue | Location | Impact |
|:--|:------|:---------|:-------|
| 10 | **`and_hms_opt(0,0,0).unwrap()`** — technically always safe but un-idiomatic | gui:221,807 · tui:134,391 | Minor; can use `NaiveTime::MIN` |
| 11 | **TUI doesn't use `SyncManager`** — rolls its own channel pattern | [tui/run.rs:59-92](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/ui/run.rs#L59-L92) | Inconsistency; duplicated sync infra |
| 12 | **Schema migration via silent `ALTER TABLE`** | [db/mod.rs:41-44, 63-66](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/db/mod.rs#L41-L44) | Fragile; use proper migration system |
| 13 | **`order_tasks_hierarchically()` only in GUI** | [gui/main.rs:228-264](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_gui/src/main.rs#L228-L264) | Feature parity gap with TUI |
| 14 | **Redundant `.to_string()` calls** | [auth/mod.rs:111](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_core/src/auth/mod.rs#L111) | Clippy-level noise |

---

## 🎯 Prioritized Action Plan — Road to 10/10

### Action 1: Extract shared `row_to_task_local()` helper in `db/mod.rs`

**Impact**: Eliminates ~120 duplicated lines, fixes 🔴#1  
**Effort**: Small (30 min)

Extract a private helper function or `impl TryFrom<&rusqlite::Row<'_>> for TaskLocal`:

```diff
+ fn row_to_task_local(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskLocal> {
+     let is_completed_int: i32 = row.get(3)?;
+     let due = row.get::<_, Option<String>>(5)?.as_ref()
+         .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
+         .map(|dt| dt.with_timezone(&Utc));
+     // ... (single canonical implementation)
+ }

  // Then in each query function:
- let task_iter = stmt.query_map([], |row| { /* 40 lines */ })?;
+ let task_iter = stmt.query_map([], row_to_task_local)?;
```

### Action 2: Move `parse_nlp_task()` and `order_tasks_hierarchically()` to `gtasks_core`

**Impact**: Eliminates ~200 duplicated lines across frontends, fixes 🔴#2 and 🟢#13  
**Effort**: Small (20 min)

These are pure business-logic functions with no UI dependencies. Move to `gtasks_core::lib.rs` or a new `gtasks_core::util` module and re-export.

### Action 3: Wrap blocking DB calls with `tokio::task::spawn_blocking`

**Impact**: Prevents async runtime stalls, fixes 🔴#3  
**Effort**: Medium (1-2 hours)

In `sync.rs::perform_single_sync()`, wrap DB-touching work:

```diff
  async fn perform_single_sync(...) -> Result<...> {
-     let mut db = Database::new("task_lists.db").map_err(|e| e.to_string())?;
-     sync_local_to_db(&mut client, &mut db).await.map_err(|e| e.to_string())?;
+     let db = tokio::task::spawn_blocking(|| Database::new("task_lists.db"))
+         .await.unwrap().map_err(|e| e.to_string())?;
      // ... similarly wrap all DB calls
  }
```

### Action 4: Split `gtasks_gui/src/main.rs` into modules

**Impact**: Makes GUI crate maintainable, fixes 🔴#4  
**Effort**: Medium (1-2 hours)

Suggested split:
| New file | Contents |
|:---|:---|
| `src/main.rs` | App entry point, CSS, `fn main()` |
| `src/app.rs` | `AppModel`, `AppInput`, `SimpleComponent impl` |
| `src/factories/task_list_row.rs` | `TaskListRow` factory |
| `src/factories/task_row.rs` | `TaskRow` factory |

### Action 5: Fix GUI `ToggleActiveTask` data loss + add `tracing` to TUI

**Impact**: Prevents due-date wipe on toggle (data integrity), logging consistency. Fixes 🟡#6 and 🟡#9  
**Effort**: Small (30 min)

For the toggle data loss, read the existing task's `due` before saving:

```diff
  // gui/main.rs ToggleActiveTask handler
+ let existing_due = if let Some(ref db) = self.db {
+     db.get_tasks_for_list(&self.list_id).ok()
+         .and_then(|tasks| tasks.iter().find(|t| t.id == task_id).map(|t| t.due))
+         .flatten()
+ } else { None };
  let updated_task = TaskLocal {
      ...
-     due: None,
+     due: existing_due,
      ...
  };
```

For TUI logging, replace 3 `println!` calls in [main.rs](file:///home/alex_insc/project/gtasks-tui/crates/gtasks_tui/src/main.rs) with `tracing::info!`.

---

## 📈 Projected Ratings After Actions 1-5

| Crate | Architecture | Cleanliness | Concurrency | DB & Sync Safety | **Overall** |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `gtasks_core` | **9/10** | **9/10** | **8/10** | **9/10** | **8.8/10** |
| `gtasks_gui`  | **8/10** | **8/10** | **8/10** | **9/10** | **8.3/10** |
| `gtasks_tui`  | **9/10** | **9/10** | **7/10** | **8/10** | **8.3/10** |

**Projected Workspace Overall: 8.5/10** — A ~1.3 point improvement from 5 targeted refactors, none requiring API changes.
