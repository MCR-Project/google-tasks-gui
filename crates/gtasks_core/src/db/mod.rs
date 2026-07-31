use crate::api::{TaskList, TaskLocal};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};

fn row_to_task_local(row: &rusqlite::Row<'_>) -> Result<TaskLocal> {
    let is_completed_int: i32 = row.get(3)?;
    let due_str: Option<String> = row.get(5)?;
    let completed_str: Option<String> = row.get(6)?;
    let updated_str: Option<String> = row.get(8)?;

    let due = due_str.as_ref().and_then(|due_str| {
        DateTime::parse_from_rfc3339(due_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let completed = completed_str.as_ref().and_then(|completed_str| {
        DateTime::parse_from_rfc3339(completed_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let updated = updated_str.as_ref().and_then(|updated_str| {
        DateTime::parse_from_rfc3339(updated_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let dirty_int: i32 = row.get(9)?;
    let is_dirty: bool = dirty_int == 1;
    let is_deleted_int: i32 = row.get(10)?;
    let is_deleted: bool = is_deleted_int == 1;

    Ok(TaskLocal {
        id: row.get(0)?,
        list_id: row.get(1)?,
        title: row.get(2)?,
        is_completed: is_completed_int != 0,
        notes: row.get(4)?,
        due,
        completed,
        parent: row.get(7)?,
        updated,
        is_dirty,
        is_deleted,
    })
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                e.to_string(),
            )))
        })
    }

    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS task_lists (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                updated TEXT,
                dirty INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        if !Self::column_exists(&conn, "task_lists", "dirty")? {
            conn.execute(
                "ALTER TABLE task_lists ADD COLUMN dirty INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                list_id TEXT NOT NULL,
                title TEXT,
                is_completed INTEGER NOT NULL,
                notes TEXT,
                due TEXT,
                completed TEXT,
                parent TEXT,
                updated TEXT,
                dirty INTEGER NOT NULL DEFAULT 0,
                is_deleted INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(list_id) REFERENCES task_lists(id)
            )",
            [],
        )?;
        if !Self::column_exists(&conn, "tasks", "is_deleted")? {
            conn.execute(
                "ALTER TABLE tasks ADD COLUMN is_deleted INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn save_task_lists(&self, task_lists: &[TaskList]) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        for task_list in task_lists {
            tx.execute(
                "INSERT INTO task_lists (id, title, updated, dirty) VALUES (?1, ?2, ?3, 0) ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated = excluded.updated, dirty = 0",
                params![task_list.id, task_list.title, task_list.updated],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_task_lists(&self) -> Result<Vec<TaskList>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT id, title, updated FROM task_lists")?;
        let task_list_iter = stmt.query_map([], |row| {
            Ok(TaskList {
                id: row.get(0)?,
                title: row.get(1)?,
                updated: row.get(2)?,
            })
        })?;

        let mut task_lists = Vec::new();
        for task_list in task_list_iter {
            task_lists.push(task_list?);
        }
        Ok(task_lists)
    }

    pub fn get_dirty_task_lists(&self) -> Result<Vec<TaskList>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, updated FROM task_lists WHERE dirty = 1 OR id LIKE 'list_%'",
        )?;
        let task_list_iter = stmt.query_map([], |row| {
            Ok(TaskList {
                id: row.get(0)?,
                title: row.get(1)?,
                updated: row.get(2)?,
            })
        })?;

        let mut task_lists = Vec::new();
        for task_list in task_list_iter {
            task_lists.push(task_list?);
        }
        Ok(task_lists)
    }

    pub fn delete_task_list_db(&self, list_id: &str) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM tasks WHERE list_id = ?1", params![list_id])?;
        tx.execute("DELETE FROM task_lists WHERE id = ?1", params![list_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn migrate_local_list_id(&self, old_list_id: &str, new_list: &TaskList) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO task_lists (id, title, updated, dirty) VALUES (?1, ?2, ?3, 0) ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated = excluded.updated, dirty = 0",
            params![new_list.id, new_list.title, new_list.updated],
        )?;
        tx.execute(
            "UPDATE tasks SET list_id = ?1 WHERE list_id = ?2",
            params![new_list.id, old_list_id],
        )?;
        tx.execute("DELETE FROM task_lists WHERE id = ?1", params![old_list_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn mark_task_deleted(&self, task_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE tasks SET is_deleted = 1, dirty = 1 WHERE id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    pub fn mark_task_clean(&self, task_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("UPDATE tasks SET dirty = 0 WHERE id = ?1", params![task_id])?;
        Ok(())
    }

    pub fn purge_task(&self, task_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![task_id])?;
        Ok(())
    }

    pub fn get_pending_deletions(&self) -> Result<Vec<TaskLocal>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("
        SELECT id, list_id, title, is_completed, notes, due, completed, parent, updated, dirty, is_deleted FROM tasks WHERE is_deleted = 1")?;

        let task_iter = stmt.query_map([], row_to_task_local)?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }
        Ok(tasks)
    }

    pub fn get_tasks_for_list(&self, list_id: &str) -> Result<Vec<TaskLocal>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("
        SELECT id, list_id, title, is_completed, notes, due, completed, parent, updated, dirty, is_deleted FROM tasks WHERE list_id = ?1 AND is_deleted = 0")?;

        let task_iter = stmt.query_map(params![list_id], row_to_task_local)?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }

        Ok(tasks)
    }

    pub fn get_all_tasks(&self) -> Result<Vec<TaskLocal>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("
        SELECT id, list_id, title, is_completed, notes, due, completed, parent, updated, dirty, is_deleted FROM tasks WHERE is_deleted = 0")?;

        let task_iter = stmt.query_map([], row_to_task_local)?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }

        Ok(tasks)
    }

    pub fn save_tasks(&self, tasks: &[TaskLocal]) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        for task in tasks {
            let due_str = task.due.map(|d| d.to_rfc3339());
            let completed_str = task.completed.map(|c| c.to_rfc3339());
            let updated_str = task.updated.map(|u| u.to_rfc3339());
            let is_completed_int = if task.is_completed { 1 } else { 0 };
            let is_dirty_int = if task.is_dirty { 1 } else { 0 };
            let is_deleted_int = if task.is_deleted { 1 } else { 0 };
            tx.execute(
                "INSERT INTO tasks (id, list_id, title, is_completed, notes, due, completed, parent, updated, dirty, is_deleted) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) 
                ON CONFLICT(id) DO UPDATE SET 
                    list_id = excluded.list_id, 
                    title = excluded.title, 
                    is_completed = excluded.is_completed, 
                    notes = excluded.notes, 
                    due = excluded.due, 
                    completed = excluded.completed, 
                    parent = excluded.parent, 
                    updated = excluded.updated,
                    dirty = excluded.dirty,
                    is_deleted = excluded.is_deleted
                WHERE excluded.dirty = 1 OR (tasks.dirty = 0 AND (excluded.updated >= tasks.updated OR tasks.updated IS NULL))",

                params![
                    task.id,
                    task.list_id,
                    task.title,
                    is_completed_int,
                    task.notes,
                    due_str,
                    completed_str,
                    task.parent,
                    updated_str,
                    is_dirty_int,
                    is_deleted_int
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_tasks_db(&self, task_ids: &[String]) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        for task_id in task_ids {
            tx.execute("DELETE FROM tasks WHERE id = ?1", params![task_id])?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_dirty_task(&self) -> Result<Vec<TaskLocal>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT id, list_id, title, is_completed, notes, due, completed, parent, updated, dirty, is_deleted FROM tasks WHERE dirty = 1 AND is_deleted = 0")?;

        let task_iter = stmt.query_map([], row_to_task_local)?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }
        Ok(tasks)
    }

    pub fn get_uncompleted_count_for_list(&self, list_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id = ?1 AND is_completed = 0 AND is_deleted = 0",
            params![list_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_all_uncompleted_count(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE is_completed = 0 AND is_deleted = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_starred_uncompleted_count(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE is_completed = 0 AND is_deleted = 0 AND title LIKE '⭐ %'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

pub trait TaskRepository: Send + Sync {
    fn save_task_lists(&self, task_lists: &[TaskList]) -> crate::Result<()>;
    fn get_task_lists(&self) -> crate::Result<Vec<TaskList>>;
    fn get_dirty_task_lists(&self) -> crate::Result<Vec<TaskList>>;
    fn delete_task_list_db(&self, list_id: &str) -> crate::Result<()>;
    fn get_tasks_for_list(&self, list_id: &str) -> crate::Result<Vec<TaskLocal>>;
    fn get_dirty_task(&self) -> crate::Result<Vec<TaskLocal>>;
    fn save_tasks(&self, tasks: &[TaskLocal]) -> crate::Result<()>;
    fn mark_task_clean(&self, task_id: &str) -> crate::Result<()>;
    fn purge_task(&self, task_id: &str) -> crate::Result<()>;
}

impl TaskRepository for Database {
    fn save_task_lists(&self, task_lists: &[TaskList]) -> crate::Result<()> {
        self.save_task_lists(task_lists).map_err(Into::into)
    }
    fn get_task_lists(&self) -> crate::Result<Vec<TaskList>> {
        self.get_task_lists().map_err(Into::into)
    }
    fn get_dirty_task_lists(&self) -> crate::Result<Vec<TaskList>> {
        self.get_dirty_task_lists().map_err(Into::into)
    }
    fn delete_task_list_db(&self, list_id: &str) -> crate::Result<()> {
        self.delete_task_list_db(list_id).map_err(Into::into)
    }
    fn get_tasks_for_list(&self, list_id: &str) -> crate::Result<Vec<TaskLocal>> {
        self.get_tasks_for_list(list_id).map_err(Into::into)
    }
    fn get_dirty_task(&self) -> crate::Result<Vec<TaskLocal>> {
        self.get_dirty_task().map_err(Into::into)
    }
    fn save_tasks(&self, tasks: &[TaskLocal]) -> crate::Result<()> {
        self.save_tasks(tasks).map_err(Into::into)
    }
    fn mark_task_clean(&self, task_id: &str) -> crate::Result<()> {
        self.mark_task_clean(task_id).map_err(Into::into)
    }
    fn purge_task(&self, task_id: &str) -> crate::Result<()> {
        self.purge_task(task_id).map_err(Into::into)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreign_keys_enabled() {
        let db = Database::new(":memory:").expect("failed to create memory db");
        let invalid_task = TaskLocal {
            id: "t1".into(),
            list_id: "nonexistent_list".into(),
            title: Some("Test".into()),
            is_completed: false,
            notes: None,
            due: None,
            completed: None,
            parent: None,
            updated: None,
            is_dirty: true,
            is_deleted: false,
        };
        let res = db.save_tasks(&[invalid_task]);
        assert!(res.is_err(), "Foreign key constraint should fail for non-existent list_id");
    }

    #[test]
    fn test_local_dirty_task_not_overwritten_by_remote() {
        let db = Database::new(":memory:").expect("failed to create memory db");
        let list = TaskList {
            id: "l1".into(),
            title: "List 1".into(),
            updated: None,
        };
        db.save_task_lists(&[list]).expect("save list failed");

        // 1. Create a local task that is dirty (dirty = 1)
        let local_task = TaskLocal {
            id: "t1".into(),
            list_id: "l1".into(),
            title: Some("Local Unsynced Title".into()),
            is_completed: false,
            notes: Some("Local Note".into()),
            due: None,
            completed: None,
            parent: None,
            updated: Some(Utc::now()),
            is_dirty: true,
            is_deleted: false,
        };
        db.save_tasks(&[local_task]).expect("save local task failed");

        // Verify it is dirty
        let dirty_tasks = db.get_dirty_task().expect("get dirty tasks failed");
        assert_eq!(dirty_tasks.len(), 1);
        assert_eq!(dirty_tasks[0].title.as_deref(), Some("Local Unsynced Title"));

        // 2. Incoming remote update with dirty = false (simulating sync_remote_to_db)
        let remote_task = TaskLocal {
            id: "t1".into(),
            list_id: "l1".into(),
            title: Some("Remote Title".into()),
            is_completed: true,
            notes: Some("Remote Note".into()),
            due: None,
            completed: None,
            parent: None,
            updated: Some(Utc::now()),
            is_dirty: false,
            is_deleted: false,
        };
        db.save_tasks(&[remote_task]).expect("save remote task failed");

        // 3. Verify local dirty task title and dirty status were preserved
        let tasks = db.get_tasks_for_list("l1").expect("get tasks failed");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title.as_deref(), Some("Local Unsynced Title"));
        assert!(tasks[0].is_dirty);

        // 4. Mark clean and save remote update
        db.mark_task_clean("t1").expect("mark task clean failed");
        let clean_tasks = db.get_dirty_task().expect("get dirty tasks failed");
        assert!(clean_tasks.is_empty());

        let remote_task_after = TaskLocal {
            id: "t1".into(),
            list_id: "l1".into(),
            title: Some("Remote Title".into()),
            is_completed: true,
            notes: Some("Remote Note".into()),
            due: None,
            completed: None,
            parent: None,
            updated: Some(Utc::now()),
            is_dirty: false,
            is_deleted: false,
        };
        db.save_tasks(&[remote_task_after]).expect("save remote task after clean failed");

        let updated_tasks = db.get_tasks_for_list("l1").expect("get tasks failed");
        assert_eq!(updated_tasks[0].title.as_deref(), Some("Remote Title"));
        assert!(!updated_tasks[0].is_dirty);
    }
}

