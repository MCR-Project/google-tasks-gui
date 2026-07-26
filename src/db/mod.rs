use crate::api::{TaskList, TaskLocal};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS task_lists (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                updated TEXT
            )",
            [],
        )?;

        self.conn.execute(
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
                FOREIGN KEY(list_id) REFERENCES task_lists(id)
            )",
            [],
        )?;
        Ok(())
    }

    pub fn save_task_lists(&mut self, task_lists: &[TaskList]) -> Result<()> {
        let tx = self.conn.transaction()?;

        for task_list in task_lists {
            tx.execute(
                "INSERT INTO task_lists (id, title, updated) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated = excluded.updated",
                params![task_list.id, task_list.title, task_list.updated],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_task_lists(&self) -> Result<Vec<TaskList>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, updated FROM task_lists")?;
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

    pub fn get_tasks_for_list(&self, list_id: &str) -> Result<Vec<TaskLocal>> {
        let mut stmt = self.conn.prepare("
        SELECT id, list_id, title, is_completed, notes, due, completed, parent, updated FROM tasks WHERE list_id = ?1")?;

        let task_iter = stmt.query_map(params![list_id], |row| {
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
            })
        })?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }

        Ok(tasks)
    }

    pub fn save_tasks(&mut self, tasks: &[TaskLocal]) -> Result<()> {
        let tx = self.conn.transaction()?;

        for task in tasks {
            // convert chrono datetime into rfc3339 string for storage in sqlite
            let due_str = task.due.map(|d| d.to_rfc3339());
            let completed_str = task.completed.map(|c| c.to_rfc3339());
            let updated_str = task.updated.map(|u| u.to_rfc3339());
            let is_completed_int = if task.is_completed { 1 } else { 0 };
            tx.execute(
                "INSERT INTO tasks (id, list_id, title, is_completed, notes, due, completed, parent, updated) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) 
                ON CONFLICT(id) DO UPDATE SET 
                    list_id = excluded.list_id, 
                    title = excluded.title, 
                    is_completed = excluded.is_completed, 
                    notes = excluded.notes, 
                    due = excluded.due, 
                    completed = excluded.completed, 
                    parent = excluded.parent, 
                    updated = excluded.updated",

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
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
