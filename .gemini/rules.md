# Project Guidelines & Learned Rules

## 1. Interaction & Workflow Rules
- **Tutorial Mode Default**: Unless the user explicitly requests direct file edits (e.g. "you can edit my file"), always provide code snippets, step-by-step guidance, and compiler error diagnostics without directly mutating user files.
- **Readability Over Complex Syntax**: Prefer clean, readable Rust type annotations (e.g. `let dirty_int: i32 = row.get(idx)?;` or `let is_dirty: bool = row.get(idx)?;`) over noisy turbofish syntax (`::<_, i32>`).

## 2. Google Tasks REST API Contracts & OAuth Security
- **Fetch Task Lists**: `GET https://www.googleapis.com/tasks/v1/users/@me/lists`
- **Fetch Tasks**: `GET https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks`
- **Create Task**: `POST https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks`
- **Toggle / Patch Task**: `PATCH https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}` (Payload: `{"status": "completed"}` or `{"status": "needsAction"}`)
- **Delete Task**: `DELETE https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}` (Returns `HTTP 204 No Content`)
- **OAuth State CSRF Security**: Always include a random `state` parameter in OAuth URLs and validate the returned `state` in the callback to prevent CSRF attacks.
- **Explicit IPv4 Loopback Listener**: Always bind local HTTP callback listeners to `127.0.0.1:<port>` rather than `localhost` to avoid hostname resolution ambiguity.

## 3. Rust Async Client Patterns
- API methods should take `&mut self` when using the `execute_with_retry` closure pattern (`Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder`) to allow in-memory access token refresh on HTTP 401 Unauthorized.

## 4. SQLite Offline Sync & Conflict Resolution Rules
- **Offline Queueing**: Include a `dirty INTEGER NOT NULL DEFAULT 0` column in `tasks` table for tracking offline edits.
- **"Last Write Wins" Conflict Resolution**: Perform timestamp conflict resolution directly in SQLite using conditional upserts:
  `ON CONFLICT(id) DO UPDATE SET ... WHERE excluded.updated >= tasks.updated OR tasks.dirty = 0`
