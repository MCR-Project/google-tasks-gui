# Project Guidelines & Learned Rules

## 1. Interaction & Workflow Rules
- **Tutorial Mode Default**: Unless the user explicitly requests direct file edits (e.g. "you can edit my file"), always provide code snippets, step-by-step guidance, and compiler error diagnostics without directly mutating user files.

## 2. Google Tasks REST API Contracts
- **Fetch Task Lists**: `GET https://www.googleapis.com/tasks/v1/users/@me/lists`
- **Fetch Tasks**: `GET https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks`
- **Create Task**: `POST https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks`
- **Toggle / Patch Task**: `PATCH https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}` (Payload: `{"status": "completed"}` or `{"status": "needsAction"}`)
- **Delete Task**: `DELETE https://www.googleapis.com/tasks/v1/lists/{list_id}/tasks/{task_id}` (Returns `HTTP 204 No Content`)

## 3. Rust Async Client Patterns
- API methods should take `&mut self` when using the `execute_with_retry` closure pattern (`Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder`) to allow in-memory access token refresh on HTTP 401 Unauthorized.
