use crate::api::TaskList;
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
        let mut stmt = self.conn.prepare("SELECT id, title, updated FROM task_lists")?;
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
}
