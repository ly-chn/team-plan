use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holiday {
    pub id: i64,
    pub date: String,
    /// "overtime" = 法定加班(调休上班), "rest" = 法定休假
    pub htype: String,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub iteration_name: String,
    pub start_date: String,
    pub end_date: String,
    pub hours_review: f64,
    pub hours_coding: f64,
    pub hours_testing: f64,
    pub hours_deploy: f64,
    pub hours_tracking: f64,
    pub hours_other: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRecord {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub start_date: String,
    pub hours: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvertimeRecord {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub start_date: String,
    pub hours: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub members_m: i64,
    pub members_n: i64,
    pub tasks_m: i64,
    pub tasks_n: i64,
    pub overtime_m: f64,
    pub overtime_n: f64,
    pub leave_m: f64,
    pub leave_n: f64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE TABLE IF NOT EXISTS holidays (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                htype TEXT NOT NULL CHECK(htype IN ('overtime','rest')),
                note TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                UNIQUE(date, htype)
            );
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES users(id),
                iteration_name TEXT NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                hours_review REAL NOT NULL DEFAULT 0,
                hours_coding REAL NOT NULL DEFAULT 0,
                hours_testing REAL NOT NULL DEFAULT 0,
                hours_deploy REAL NOT NULL DEFAULT 0,
                hours_tracking REAL NOT NULL DEFAULT 0,
                hours_other REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE TABLE IF NOT EXISTS leave_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES users(id),
                start_date TEXT NOT NULL,
                hours REAL NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE TABLE IF NOT EXISTS overtime_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL REFERENCES users(id),
                start_date TEXT NOT NULL,
                hours REAL NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_user ON tasks(user_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_dates ON tasks(start_date, end_date);
            CREATE INDEX IF NOT EXISTS idx_leave_user ON leave_records(user_id);
            CREATE INDEX IF NOT EXISTS idx_overtime_user ON overtime_records(user_id);
            CREATE INDEX IF NOT EXISTS idx_holidays_date ON holidays(date);"
        )?;
        Ok(())
    }

    // ─── Users ───

    pub fn create_user(&self, username: &str, name: &str, password_hash: &str) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO users (username, name, password_hash) VALUES (?1, ?2, ?3)",
            params![username, name, password_hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_user_by_username(&self, username: &str) -> SqlResult<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, name, password_hash, created_at FROM users WHERE username = ?1"
        )?;
        let mut rows = stmt.query_map(params![username], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                name: row.get(2)?,
                password_hash: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn get_user_by_id(&self, id: i64) -> SqlResult<Option<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, name, password_hash, created_at FROM users WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                name: row.get(2)?,
                password_hash: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn list_users(&self) -> SqlResult<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, name, password_hash, created_at FROM users ORDER BY id"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                name: row.get(2)?,
                password_hash: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    // ─── Holidays ───

    pub fn import_holidays(&self, holidays: &[(String, String, String)]) -> SqlResult<usize> {
        let mut count = 0;
        for (date, htype, note) in holidays {
            let r = self.conn.execute(
                "INSERT OR REPLACE INTO holidays (date, htype, note) VALUES (?1, ?2, ?3)",
                params![date, htype, note],
            )?;
            count += r;
        }
        Ok(count)
    }

    pub fn list_holidays(&self) -> SqlResult<Vec<Holiday>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, date, htype, note, created_at FROM holidays ORDER BY date"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Holiday {
                id: row.get(0)?,
                date: row.get(1)?,
                htype: row.get(2)?,
                note: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_holiday(&self, id: i64) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM holidays WHERE id = ?1", params![id])
    }

    // ─── Tasks ───

    pub fn create_task(
        &self,
        user_id: i64,
        iteration_name: &str,
        start_date: &str,
        end_date: &str,
        hours_review: f64,
        hours_coding: f64,
        hours_testing: f64,
        hours_deploy: f64,
        hours_tracking: f64,
        hours_other: f64,
    ) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO tasks (user_id, iteration_name, start_date, end_date,
             hours_review, hours_coding, hours_testing, hours_deploy, hours_tracking, hours_other)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                user_id, iteration_name, start_date, end_date,
                hours_review, hours_coding, hours_testing, hours_deploy, hours_tracking, hours_other,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_task(
        &self,
        id: i64,
        user_id: i64,
        iteration_name: &str,
        start_date: &str,
        end_date: &str,
        hours_review: f64,
        hours_coding: f64,
        hours_testing: f64,
        hours_deploy: f64,
        hours_tracking: f64,
        hours_other: f64,
    ) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE tasks SET user_id=?1, iteration_name=?2, start_date=?3, end_date=?4,
             hours_review=?5, hours_coding=?6, hours_testing=?7, hours_deploy=?8,
             hours_tracking=?9, hours_other=?10, updated_at=datetime('now','localtime')
             WHERE id=?11",
            params![
                user_id, iteration_name, start_date, end_date,
                hours_review, hours_coding, hours_testing, hours_deploy, hours_tracking, hours_other,
                id,
            ],
        )
    }

    pub fn delete_task(&self, id: i64) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
    }

    pub fn list_tasks(&self) -> SqlResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.user_id, u.name, t.iteration_name, t.start_date, t.end_date,
                    t.hours_review, t.hours_coding, t.hours_testing, t.hours_deploy,
                    t.hours_tracking, t.hours_other, t.created_at, t.updated_at
             FROM tasks t JOIN users u ON t.user_id = u.id
             ORDER BY t.start_date DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_name: row.get(2)?,
                iteration_name: row.get(3)?,
                start_date: row.get(4)?,
                end_date: row.get(5)?,
                hours_review: row.get(6)?,
                hours_coding: row.get(7)?,
                hours_testing: row.get(8)?,
                hours_deploy: row.get(9)?,
                hours_tracking: row.get(10)?,
                hours_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_tasks_visible(&self, vis_start: &str, vis_end: &str) -> SqlResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.user_id, u.name, t.iteration_name, t.start_date, t.end_date,
                    t.hours_review, t.hours_coding, t.hours_testing, t.hours_deploy,
                    t.hours_tracking, t.hours_other, t.created_at, t.updated_at
             FROM tasks t JOIN users u ON t.user_id = u.id
             WHERE t.start_date <= ?2 AND t.end_date >= ?1
             ORDER BY t.start_date"
        )?;
        let rows = stmt.query_map(params![vis_start, vis_end], |row| {
            Ok(Task {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_name: row.get(2)?,
                iteration_name: row.get(3)?,
                start_date: row.get(4)?,
                end_date: row.get(5)?,
                hours_review: row.get(6)?,
                hours_coding: row.get(7)?,
                hours_testing: row.get(8)?,
                hours_deploy: row.get(9)?,
                hours_tracking: row.get(10)?,
                hours_other: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn recent_iteration_names(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT iteration_name FROM tasks
             WHERE created_at >= date('now','-60 days')
             ORDER BY created_at DESC LIMIT 20"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    // ─── Leave ───

    pub fn create_leave(&self, user_id: i64, start_date: &str, hours: f64) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO leave_records (user_id, start_date, hours) VALUES (?1, ?2, ?3)",
            params![user_id, start_date, hours],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_leave(&self, id: i64, user_id: i64, start_date: &str, hours: f64) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE leave_records SET user_id=?1, start_date=?2, hours=?3 WHERE id=?4",
            params![user_id, start_date, hours, id],
        )
    }

    pub fn delete_leave(&self, id: i64) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM leave_records WHERE id = ?1", params![id])
    }

    pub fn list_leave(&self) -> SqlResult<Vec<LeaveRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.id, l.user_id, u.name, l.start_date, l.hours, l.created_at
             FROM leave_records l JOIN users u ON l.user_id = u.id
             ORDER BY l.start_date DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LeaveRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_name: row.get(2)?,
                start_date: row.get(3)?,
                hours: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    // ─── Overtime ───

    pub fn create_overtime(&self, user_id: i64, start_date: &str, hours: f64) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO overtime_records (user_id, start_date, hours) VALUES (?1, ?2, ?3)",
            params![user_id, start_date, hours],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_overtime(&self, id: i64, user_id: i64, start_date: &str, hours: f64) -> SqlResult<usize> {
        self.conn.execute(
            "UPDATE overtime_records SET user_id=?1, start_date=?2, hours=?3 WHERE id=?4",
            params![user_id, start_date, hours, id],
        )
    }

    pub fn delete_overtime(&self, id: i64) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM overtime_records WHERE id = ?1", params![id])
    }

    pub fn list_overtime(&self) -> SqlResult<Vec<OvertimeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.id, o.user_id, u.name, o.start_date, o.hours, o.created_at
             FROM overtime_records o JOIN users u ON o.user_id = u.id
             ORDER BY o.start_date DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(OvertimeRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_name: row.get(2)?,
                start_date: row.get(3)?,
                hours: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    // ─── Reports ───

    pub fn get_report(&self) -> SqlResult<Report> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        // members: who has tasks with end_date >= today
        let members_m: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT user_id) FROM tasks WHERE end_date >= ?1",
            params![today],
            |r| r.get(0),
        ).unwrap_or(0);
        let members_n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM users", [], |r| r.get(0)
        ).unwrap_or(0);

        // tasks: not yet ended vs total
        let tasks_m: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE end_date >= ?1",
            params![today],
            |r| r.get(0),
        ).unwrap_or(0);
        let tasks_n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks", [], |r| r.get(0)
        ).unwrap_or(0);

        // overtime: future vs total
        let overtime_m: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(hours),0) FROM overtime_records WHERE start_date >= ?1",
            params![today],
            |r| r.get(0),
        ).unwrap_or(0.0);
        let overtime_n: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(hours),0) FROM overtime_records", [], |r| r.get(0)
        ).unwrap_or(0.0);

        // leave: future vs total
        let leave_m: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(hours),0) FROM leave_records WHERE start_date >= ?1",
            params![today],
            |r| r.get(0),
        ).unwrap_or(0.0);
        let leave_n: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(hours),0) FROM leave_records", [], |r| r.get(0)
        ).unwrap_or(0.0);

        Ok(Report {
            members_m, members_n,
            tasks_m, tasks_n,
            overtime_m, overtime_n,
            leave_m, leave_n,
        })
    }
}
