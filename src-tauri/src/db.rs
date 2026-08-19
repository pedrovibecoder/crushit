use crate::error::Result;
use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        Ok(Db(Mutex::new(conn)))
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned lock means an earlier command panicked mid-query; the
        // connection itself is still usable, so recover rather than cascade.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

const SCHEMA: &str = r#"
CREATE TABLE projects (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  path           TEXT NOT NULL UNIQUE,
  name           TEXT NOT NULL,
  is_git         INTEGER NOT NULL DEFAULT 0,
  branch         TEXT,
  created_at     INTEGER NOT NULL,
  last_opened_at INTEGER NOT NULL
);

CREATE TABLE goals (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title      TEXT NOT NULL,
  prompt     TEXT NOT NULL,
  status     TEXT NOT NULL DEFAULT 'draft',
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_goals_project ON goals(project_id);

CREATE TABLE tasks (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id       INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  goal_id          INTEGER REFERENCES goals(id) ON DELETE SET NULL,
  title            TEXT NOT NULL,
  description      TEXT,
  category         TEXT NOT NULL DEFAULT 'task',
  status           TEXT NOT NULL DEFAULT 'ready',
  position         INTEGER NOT NULL DEFAULT 0,
  estimate_minutes INTEGER,
  is_ai_generated  INTEGER NOT NULL DEFAULT 0,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  completed_at     INTEGER
);
CREATE INDEX idx_tasks_project ON tasks(project_id, position);

CREATE TABLE task_acceptance_criteria (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  text     TEXT NOT NULL,
  is_met   INTEGER NOT NULL DEFAULT 0,
  position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_criteria_task ON task_acceptance_criteria(task_id, position);

CREATE TABLE task_files (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  path    TEXT NOT NULL
);
CREATE INDEX idx_task_files_task ON task_files(task_id);

CREATE TABLE task_dependencies (
  task_id            INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on_task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on_task_id)
);

CREATE TABLE codex_threads (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id          INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  codex_thread_id  TEXT NOT NULL,
  status           TEXT NOT NULL DEFAULT 'idle',
  started_at       INTEGER NOT NULL,
  last_activity_at INTEGER NOT NULL
);
CREATE INDEX idx_threads_task ON codex_threads(task_id);

CREATE TABLE focus_sessions (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id         INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  started_at      INTEGER NOT NULL,
  ended_at        INTEGER,
  planned_seconds INTEGER NOT NULL,
  elapsed_seconds INTEGER NOT NULL DEFAULT 0,
  completed       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_focus_task ON focus_sessions(task_id);

CREATE TABLE task_verifications (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id         INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at      INTEGER NOT NULL,
  satisfied_count INTEGER NOT NULL DEFAULT 0,
  total_count     INTEGER NOT NULL DEFAULT 0,
  summary         TEXT,
  raw_result      TEXT
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

/// Goals gained a Codex-produced plan once planning arrived.
const ADD_GOAL_PLANNING: &str = r#"
ALTER TABLE goals ADD COLUMN codex_thread_id TEXT;
ALTER TABLE goals ADD COLUMN plan_json TEXT;
ALTER TABLE goals ADD COLUMN error TEXT;
ALTER TABLE goals ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
"#;

/// Plans can come from more than one coding agent.
const ADD_GOAL_AGENT: &str = r#"
ALTER TABLE goals ADD COLUMN agent TEXT NOT NULL DEFAULT 'codex';
"#;

/// A task's conversation belongs to the agent that started it; resuming a
/// Claude session through Codex would be meaningless.
const ADD_THREAD_AGENT: &str = r#"
ALTER TABLE codex_threads ADD COLUMN agent TEXT NOT NULL DEFAULT 'codex';
"#;

/// Categories used to be a fixed enum in the binary. They are rows now, so the
/// developer can name their own. The built-in set collapses to Task and Bug;
/// every task that carried one of the old labels lands on Task.
const ADD_CATEGORIES: &str = r#"
CREATE TABLE categories (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  slug     TEXT NOT NULL UNIQUE,
  label    TEXT NOT NULL,
  color    TEXT NOT NULL,
  position INTEGER NOT NULL DEFAULT 0
);
INSERT INTO categories (slug, label, color, position) VALUES
  ('task', 'Task', '#4a9bf5', 0),
  ('bug', 'Bug', '#e0609b', 1);
UPDATE tasks SET category = 'task' WHERE category <> 'bug';
"#;

/// Tasks gained a relative size alongside the clock estimate, so a plan can be
/// weighed as well as scheduled.
const ADD_STORY_POINTS: &str = r#"
ALTER TABLE tasks ADD COLUMN story_points INTEGER;
"#;

/// Tasks belong to a day, so the list can be walked back and forward through
/// them. Existing tasks take the day they were made, read on the local clock —
/// which day a task belongs to is a question about the developer's calendar,
/// not about UTC.
const ADD_PLANNED_FOR: &str = r#"
ALTER TABLE tasks ADD COLUMN planned_for TEXT NOT NULL DEFAULT '';
UPDATE tasks SET planned_for = date(created_at, 'unixepoch', 'localtime');
"#;

/// Migrations are applied in order; `user_version` records how many have run.
const MIGRATIONS: &[&str] = &[
    SCHEMA,
    ADD_GOAL_PLANNING,
    ADD_GOAL_AGENT,
    ADD_THREAD_AGENT,
    ADD_CATEGORIES,
    ADD_STORY_POINTS,
    ADD_PLANNED_FOR,
];

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        conn.execute_batch(migration)?;
        conn.pragma_update(None, "user_version", (index + 1) as i64)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- projects

fn project_from_row(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        path: row.get("path")?,
        name: row.get("name")?,
        is_git: row.get::<_, i64>("is_git")? != 0,
        branch: row.get("branch")?,
        created_at: row.get("created_at")?,
        last_opened_at: row.get("last_opened_at")?,
    })
}

pub fn list_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt =
        conn.prepare("SELECT * FROM projects ORDER BY last_opened_at DESC, name COLLATE NOCASE")?;
    let rows = stmt.query_map([], project_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn find_project(conn: &Connection, id: i64) -> Result<Option<Project>> {
    Ok(conn
        .query_row("SELECT * FROM projects WHERE id = ?1", [id], project_from_row)
        .optional()?)
}

pub fn find_project_by_path(conn: &Connection, path: &str) -> Result<Option<Project>> {
    Ok(conn
        .query_row(
            "SELECT * FROM projects WHERE path = ?1",
            [path],
            project_from_row,
        )
        .optional()?)
}

pub fn upsert_project(
    conn: &Connection,
    path: &str,
    name: &str,
    is_git: bool,
    branch: Option<&str>,
) -> Result<Project> {
    let ts = now();
    conn.execute(
        "INSERT INTO projects (path, name, is_git, branch, created_at, last_opened_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(path) DO UPDATE SET
           name = excluded.name,
           is_git = excluded.is_git,
           branch = excluded.branch,
           last_opened_at = excluded.last_opened_at",
        params![path, name, is_git as i64, branch, ts],
    )?;
    find_project_by_path(conn, path)?.ok_or(crate::error::Error::NotFound("project"))
}

pub fn touch_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE projects SET last_opened_at = ?2 WHERE id = ?1",
        params![id, now()],
    )?;
    Ok(())
}

pub fn delete_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

// -------------------------------------------------------------- categories

fn category_from_row(row: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get("id")?,
        slug: row.get("slug")?,
        label: row.get("label")?,
        color: row.get("color")?,
        position: row.get("position")?,
    })
}

pub fn list_categories(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare("SELECT * FROM categories ORDER BY position, id")?;
    let rows = stmt.query_map([], category_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn find_category(conn: &Connection, id: i64) -> Result<Option<Category>> {
    Ok(conn
        .query_row("SELECT * FROM categories WHERE id = ?1", [id], category_from_row)
        .optional()?)
}

/// The slug a task falls back to: the first category in the list. There is
/// always at least one, because the last one cannot be deleted.
pub fn default_category(conn: &Connection) -> Result<String> {
    Ok(conn
        .query_row(
            "SELECT slug FROM categories ORDER BY position, id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_else(|| "task".to_string()))
}

/// Every slug, in display order. The planner offers these to the agent.
pub fn category_slugs(conn: &Connection) -> Result<Vec<String>> {
    Ok(list_categories(conn)?
        .into_iter()
        .map(|category| category.slug)
        .collect())
}

/// Maps whatever the caller asked for onto a category that exists. A slug the
/// developer has since deleted falls back rather than failing the write.
pub fn resolve_category(conn: &Connection, raw: Option<&str>) -> Result<String> {
    let Some(slug) = raw.map(str::trim).filter(|slug| !slug.is_empty()) else {
        return default_category(conn);
    };
    let known: Option<String> = conn
        .query_row("SELECT slug FROM categories WHERE slug = ?1", [slug], |row| {
            row.get(0)
        })
        .optional()?;
    match known {
        Some(slug) => Ok(slug),
        None => default_category(conn),
    }
}

pub fn create_category(conn: &Connection, label: &str, color: &str) -> Result<Category> {
    let label = label.trim();
    if label.is_empty() {
        return Err(crate::error::Error::invalid("a category needs a name"));
    }
    let slug = slugify(label);
    if slug.is_empty() {
        return Err(crate::error::Error::invalid(
            "that name has no letters or numbers in it",
        ));
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE slug = ?1)",
        [&slug],
        |row| Ok(row.get::<_, i64>(0)? != 0),
    )?;
    if taken {
        return Err(crate::error::Error::invalid(format!(
            "there is already a category called {label}"
        )));
    }
    let next_position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM categories",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO categories (slug, label, color, position) VALUES (?1, ?2, ?3, ?4)",
        params![slug, label, color.trim(), next_position],
    )?;
    find_category(conn, conn.last_insert_rowid())?.ok_or(crate::error::Error::NotFound("category"))
}

/// Renaming leaves the slug alone, so the tasks already filed under it keep
/// their category and simply show the new name.
pub fn update_category(conn: &Connection, id: i64, patch: &CategoryPatch) -> Result<Category> {
    if find_category(conn, id)?.is_none() {
        return Err(crate::error::Error::NotFound("category"));
    }
    if let Some(label) = &patch.label {
        let label = label.trim();
        if label.is_empty() {
            return Err(crate::error::Error::invalid("a category needs a name"));
        }
        conn.execute(
            "UPDATE categories SET label = ?2 WHERE id = ?1",
            params![id, label],
        )?;
    }
    if let Some(color) = &patch.color {
        conn.execute(
            "UPDATE categories SET color = ?2 WHERE id = ?1",
            params![id, color.trim()],
        )?;
    }
    find_category(conn, id)?.ok_or(crate::error::Error::NotFound("category"))
}

/// Deleting a category moves its tasks onto whichever category is left at the
/// top of the list. The last one stays: a task always has a category.
pub fn delete_category(conn: &Connection, id: i64) -> Result<Vec<Category>> {
    let category = find_category(conn, id)?.ok_or(crate::error::Error::NotFound("category"))?;
    let remaining = list_categories(conn)?;
    if remaining.len() <= 1 {
        return Err(crate::error::Error::invalid(
            "keep at least one category — tasks need something to be filed under",
        ));
    }
    let fallback = remaining
        .iter()
        .find(|other| other.id != id)
        .map(|other| other.slug.clone())
        .unwrap_or_else(|| "task".to_string());
    conn.execute(
        "UPDATE tasks SET category = ?2 WHERE category = ?1",
        params![category.slug, fallback],
    )?;
    conn.execute("DELETE FROM categories WHERE id = ?1", [id])?;
    list_categories(conn)
}

// ------------------------------------------------------------------- tasks

fn criteria_for(conn: &Connection, task_id: i64) -> Result<Vec<AcceptanceCriterion>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, text, is_met, position
         FROM task_acceptance_criteria WHERE task_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([task_id], |row| {
        Ok(AcceptanceCriterion {
            id: row.get(0)?,
            task_id: row.get(1)?,
            text: row.get(2)?,
            is_met: row.get::<_, i64>(3)? != 0,
            position: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn files_for(conn: &Connection, task_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM task_files WHERE task_id = ?1 ORDER BY id")?;
    let rows = stmt.query_map([task_id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn dependencies_for(conn: &Connection, task_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT depends_on_task_id FROM task_dependencies WHERE task_id = ?1 ORDER BY depends_on_task_id",
    )?;
    let rows = stmt.query_map([task_id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Time spent and sessions taken, in one pass over the task's sessions.
fn focus_summary_for(conn: &Connection, task_id: i64) -> Result<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(elapsed_seconds), 0), COUNT(*)
         FROM focus_sessions WHERE task_id = ?1",
        [task_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

fn task_from_row(conn: &Connection, row: &Row) -> Result<Task> {
    let id: i64 = row.get("id")?;
    let summary = focus_summary_for(conn, id)?;
    Ok(Task {
        id,
        project_id: row.get("project_id")?,
        goal_id: row.get("goal_id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        category: row.get("category")?,
        status: TaskStatus::parse_lenient(&row.get::<_, String>("status")?),
        position: row.get("position")?,
        estimate_minutes: row.get("estimate_minutes")?,
        story_points: row.get("story_points")?,
        planned_for: row.get("planned_for")?,
        is_ai_generated: row.get::<_, i64>("is_ai_generated")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        criteria: criteria_for(conn, id)?,
        files: files_for(conn, id)?,
        depends_on: dependencies_for(conn, id)?,
        focus_seconds: summary.0,
        focus_sessions: summary.1,
    })
}

pub fn list_tasks(conn: &Connection, project_id: i64) -> Result<Vec<Task>> {
    let mut stmt =
        conn.prepare("SELECT id FROM tasks WHERE project_id = ?1 ORDER BY position, id")?;
    let ids: Vec<i64> = stmt
        .query_map([project_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ids.into_iter()
        .map(|id| get_task(conn, id))
        .collect::<Result<Vec<_>>>()
}

pub fn get_task(conn: &Connection, id: i64) -> Result<Task> {
    let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query([id])?;
    match rows.next()? {
        Some(row) => task_from_row(conn, row),
        None => Err(crate::error::Error::NotFound("task")),
    }
}

fn replace_criteria(conn: &Connection, task_id: i64, texts: &[String]) -> Result<()> {
    // Preserve the met/unmet state of criteria whose text is unchanged.
    let previous: Vec<(String, bool)> = criteria_for(conn, task_id)?
        .into_iter()
        .map(|c| (c.text, c.is_met))
        .collect();
    conn.execute(
        "DELETE FROM task_acceptance_criteria WHERE task_id = ?1",
        [task_id],
    )?;
    for (index, text) in texts.iter().enumerate() {
        let was_met = previous
            .iter()
            .find(|(prev, _)| prev == text)
            .map(|(_, met)| *met)
            .unwrap_or(false);
        conn.execute(
            "INSERT INTO task_acceptance_criteria (task_id, text, is_met, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![task_id, text, was_met as i64, index as i64],
        )?;
    }
    Ok(())
}

fn replace_files(conn: &Connection, task_id: i64, paths: &[String]) -> Result<()> {
    conn.execute("DELETE FROM task_files WHERE task_id = ?1", [task_id])?;
    for path in paths {
        conn.execute(
            "INSERT INTO task_files (task_id, path) VALUES (?1, ?2)",
            params![task_id, path],
        )?;
    }
    Ok(())
}

fn replace_dependencies(conn: &Connection, task_id: i64, depends_on: &[i64]) -> Result<()> {
    conn.execute("DELETE FROM task_dependencies WHERE task_id = ?1", [task_id])?;
    for other in depends_on {
        if *other == task_id {
            return Err(crate::error::Error::invalid("a task cannot depend on itself"));
        }
        conn.execute(
            "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_task_id) VALUES (?1, ?2)",
            params![task_id, other],
        )?;
    }
    Ok(())
}

/// Today on the developer's own clock, as `YYYY-MM-DD`.
pub fn today(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?)
}

/// A day to file a task under. Anything that is not a plain date falls back to
/// today rather than being stored and later failing to match anything.
fn planned_day(conn: &Connection, raw: Option<&str>) -> Result<String> {
    let looks_like_a_date = |day: &str| {
        day.len() == 10
            && day.as_bytes()[4] == b'-'
            && day.as_bytes()[7] == b'-'
            && day.chars().filter(char::is_ascii_digit).count() == 8
    };
    match raw.map(str::trim).filter(|day| looks_like_a_date(day)) {
        Some(day) => Ok(day.to_string()),
        None => today(conn),
    }
}

pub fn create_task(conn: &Connection, input: &NewTask) -> Result<Task> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(crate::error::Error::invalid("task title cannot be empty"));
    }
    let ts = now();
    let next_position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE project_id = ?1",
        [input.project_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks
           (project_id, goal_id, title, description, category, status, position,
            estimate_minutes, story_points, is_ai_generated, planned_for,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            input.project_id,
            input.goal_id,
            title,
            input.description.as_deref(),
            resolve_category(conn, input.category.as_deref())?,
            input.status.unwrap_or_default(),
            next_position,
            input.estimate_minutes,
            input.story_points.and_then(nearest_story_points),
            input.is_ai_generated.unwrap_or(false) as i64,
            planned_day(conn, input.planned_for.as_deref())?,
            ts,
        ],
    )?;
    let id = conn.last_insert_rowid();
    if let Some(criteria) = &input.criteria {
        replace_criteria(conn, id, criteria)?;
    }
    if let Some(files) = &input.files {
        replace_files(conn, id, files)?;
    }
    get_task(conn, id)
}

pub fn update_task(conn: &Connection, id: i64, patch: &TaskPatch) -> Result<Task> {
    let existing = get_task(conn, id)?;

    if let Some(title) = &patch.title {
        if title.trim().is_empty() {
            return Err(crate::error::Error::invalid("task title cannot be empty"));
        }
        conn.execute(
            "UPDATE tasks SET title = ?2 WHERE id = ?1",
            params![id, title.trim()],
        )?;
    }
    if let Some(description) = &patch.description {
        let value = description.trim();
        conn.execute(
            "UPDATE tasks SET description = ?2 WHERE id = ?1",
            params![id, (!value.is_empty()).then_some(value)],
        )?;
    }
    if let Some(category) = &patch.category {
        conn.execute(
            "UPDATE tasks SET category = ?2 WHERE id = ?1",
            params![id, resolve_category(conn, Some(category))?],
        )?;
    }
    if let Some(status) = patch.status {
        // Entering `completed` stamps the time; leaving it clears the stamp.
        let completed_at = match (status.is_done(), existing.completed_at) {
            (true, Some(previous)) => Some(previous),
            (true, None) => Some(now()),
            (false, _) => None,
        };
        conn.execute(
            "UPDATE tasks SET status = ?2, completed_at = ?3 WHERE id = ?1",
            params![id, status, completed_at],
        )?;
    }
    if let Some(estimate) = patch.estimate_minutes {
        conn.execute(
            "UPDATE tasks SET estimate_minutes = ?2 WHERE id = ?1",
            params![id, (estimate > 0).then_some(estimate)],
        )?;
    }
    if let Some(day) = &patch.planned_for {
        conn.execute(
            "UPDATE tasks SET planned_for = ?2 WHERE id = ?1",
            params![id, planned_day(conn, Some(day))?],
        )?;
    }
    if let Some(points) = patch.story_points {
        conn.execute(
            "UPDATE tasks SET story_points = ?2 WHERE id = ?1",
            params![id, nearest_story_points(points)],
        )?;
    }
    if let Some(criteria) = &patch.criteria {
        replace_criteria(conn, id, criteria)?;
    }
    if let Some(files) = &patch.files {
        replace_files(conn, id, files)?;
    }
    if let Some(depends_on) = &patch.depends_on {
        replace_dependencies(conn, id, depends_on)?;
    }

    conn.execute(
        "UPDATE tasks SET updated_at = ?2 WHERE id = ?1",
        params![id, now()],
    )?;
    get_task(conn, id)
}

pub fn delete_task(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
    Ok(())
}

pub fn reorder_tasks(conn: &Connection, project_id: i64, ordered_ids: &[i64]) -> Result<()> {
    for (index, id) in ordered_ids.iter().enumerate() {
        conn.execute(
            "UPDATE tasks SET position = ?3 WHERE id = ?1 AND project_id = ?2",
            params![id, project_id, index as i64],
        )?;
    }
    Ok(())
}

pub fn set_criterion_met(conn: &Connection, id: i64, is_met: bool) -> Result<i64> {
    let task_id: i64 = conn
        .query_row(
            "SELECT task_id FROM task_acceptance_criteria WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(crate::error::Error::NotFound("criterion"))?;
    conn.execute(
        "UPDATE task_acceptance_criteria SET is_met = ?2 WHERE id = ?1",
        params![id, is_met as i64],
    )?;
    Ok(task_id)
}

// ----------------------------------------------------------- agent threads

/// Records the conversation an agent used for a task, so a later run can pick
/// up where the last one stopped instead of starting from nothing.
pub fn save_task_thread(
    conn: &Connection,
    task_id: i64,
    agent: crate::agent::Agent,
    thread_id: &str,
    status: &str,
) -> Result<()> {
    let ts = now();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM codex_threads WHERE task_id = ?1 AND agent = ?2",
            params![task_id, agent],
            |row| row.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => conn.execute(
            "UPDATE codex_threads
             SET codex_thread_id = ?2, status = ?3, last_activity_at = ?4
             WHERE id = ?1",
            params![id, thread_id, status, ts],
        )?,
        None => conn.execute(
            "INSERT INTO codex_threads
               (task_id, agent, codex_thread_id, status, started_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![task_id, agent, thread_id, status, ts],
        )?,
    };
    Ok(())
}

/// The conversation this agent last used for the task, if there is one.
pub fn task_thread(
    conn: &Connection,
    task_id: i64,
    agent: crate::agent::Agent,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT codex_thread_id FROM codex_threads WHERE task_id = ?1 AND agent = ?2",
            params![task_id, agent],
            |row| row.get(0),
        )
        .optional()?)
}

// ----------------------------------------------------------- focus sessions

pub fn open_focus_session(conn: &Connection, task_id: i64, planned_seconds: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO focus_sessions (task_id, started_at, planned_seconds) VALUES (?1, ?2, ?3)",
        params![task_id, now(), planned_seconds],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn record_focus_progress(
    conn: &Connection,
    session_id: i64,
    elapsed_seconds: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE focus_sessions SET elapsed_seconds = ?2 WHERE id = ?1",
        params![session_id, elapsed_seconds],
    )?;
    Ok(())
}

pub fn close_focus_session(
    conn: &Connection,
    session_id: i64,
    elapsed_seconds: i64,
    completed: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE focus_sessions
         SET ended_at = ?2, elapsed_seconds = ?3, completed = ?4
         WHERE id = ?1",
        params![session_id, now(), elapsed_seconds, completed as i64],
    )?;
    Ok(())
}

/// A session left open by a crash or quit, used to restore the timer on launch.
pub fn latest_open_focus_session(conn: &Connection) -> Result<Option<(i64, i64, i64, i64)>> {
    Ok(conn
        .query_row(
            "SELECT id, task_id, planned_seconds, elapsed_seconds
             FROM focus_sessions WHERE ended_at IS NULL
             ORDER BY started_at DESC, id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?)
}

/// Any older open session is stale once one has been restored.
pub fn close_stale_focus_sessions(conn: &Connection, except_id: Option<i64>) -> Result<()> {
    conn.execute(
        "UPDATE focus_sessions
         SET ended_at = COALESCE(ended_at, started_at + elapsed_seconds)
         WHERE ended_at IS NULL AND id IS NOT ?1",
        params![except_id],
    )?;
    Ok(())
}


// ------------------------------------------------------------------- goals

/// A goal's display name is the first line of what the developer typed.
fn title_from_prompt(prompt: &str) -> String {
    let first = prompt
        .trim()
        .lines()
        .next()
        .unwrap_or("Untitled goal")
        .trim();
    let sentence = first.split_once(". ").map(|(head, _)| head).unwrap_or(first);
    let title: String = sentence.chars().take(72).collect();
    if title.trim().is_empty() {
        "Untitled goal".to_string()
    } else {
        title.trim().trim_end_matches('.').to_string()
    }
}

fn goal_from_row(row: &Row) -> Result<Goal> {
    let plan_json: Option<String> = row.get("plan_json")?;
    Ok(Goal {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        prompt: row.get("prompt")?,
        status: GoalStatus::parse_lenient(&row.get::<_, String>("status")?),
        agent: crate::agent::Agent::parse_lenient(&row.get::<_, String>("agent")?),
        codex_thread_id: row.get("codex_thread_id")?,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        // A plan written by an older build that no longer parses is treated as
        // absent rather than poisoning the whole read.
        plan: plan_json.and_then(|raw| serde_json::from_str(&raw).ok()),
    })
}

pub fn create_goal(
    conn: &Connection,
    project_id: i64,
    prompt: &str,
    agent: crate::agent::Agent,
) -> Result<Goal> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(crate::error::Error::invalid("describe what you want to build"));
    }
    let ts = now();
    conn.execute(
        "INSERT INTO goals (project_id, title, prompt, status, agent, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'draft', ?4, ?5, ?5)",
        params![project_id, title_from_prompt(prompt), prompt, agent, ts],
    )?;
    get_goal(conn, conn.last_insert_rowid())
}

pub fn get_goal(conn: &Connection, id: i64) -> Result<Goal> {
    let mut stmt = conn.prepare("SELECT * FROM goals WHERE id = ?1")?;
    let mut rows = stmt.query([id])?;
    match rows.next()? {
        Some(row) => goal_from_row(row),
        None => Err(crate::error::Error::NotFound("goal")),
    }
}

/// The goal the Goal screen should reopen on: the most recent one that still
/// wants a decision, otherwise the most recent of any kind.
pub fn latest_goal(conn: &Connection, project_id: i64) -> Result<Option<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM goals WHERE project_id = ?1
         ORDER BY (status IN ('ready', 'analyzing', 'failed')) DESC, created_at DESC, id DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query([project_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(goal_from_row(row)?)),
        None => Ok(None),
    }
}

pub fn set_goal_status(conn: &Connection, id: i64, status: GoalStatus) -> Result<Goal> {
    conn.execute(
        "UPDATE goals SET status = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, status, now()],
    )?;
    get_goal(conn, id)
}

pub fn set_goal_plan(
    conn: &Connection,
    id: i64,
    thread_id: &str,
    plan: &crate::plan::Plan,
) -> Result<Goal> {
    conn.execute(
        "UPDATE goals
         SET status = 'ready', plan_json = ?2, codex_thread_id = ?3, error = NULL, updated_at = ?4
         WHERE id = ?1",
        params![id, serde_json::to_string(plan)?, thread_id, now()],
    )?;
    get_goal(conn, id)
}

pub fn set_goal_failed(conn: &Connection, id: i64, error: &str) -> Result<Goal> {
    conn.execute(
        "UPDATE goals SET status = 'failed', error = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, error, now()],
    )?;
    get_goal(conn, id)
}

pub fn delete_goal(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM goals WHERE id = ?1", [id])?;
    Ok(())
}

/// Writes an accepted plan out as real tasks, translating the plan's own
/// 1-based `dependsOn` positions into task ids.
pub fn create_tasks_from_plan(conn: &Connection, goal: &Goal) -> Result<Vec<Task>> {
    let plan = goal
        .plan
        .as_ref()
        .ok_or_else(|| crate::error::Error::invalid("that goal has no plan to add"))?;

    let mut created: Vec<Task> = Vec::with_capacity(plan.tasks.len());
    for planned in &plan.tasks {
        let task = create_task(
            conn,
            &NewTask {
                project_id: goal.project_id,
                goal_id: Some(goal.id),
                title: planned.title.clone(),
                description: (!planned.description.is_empty())
                    .then(|| planned.description.clone()),
                category: Some(planned.category.clone()),
                status: Some(TaskStatus::Ready),
                estimate_minutes: (planned.estimate_minutes > 0)
                    .then_some(planned.estimate_minutes),
                story_points: (planned.story_points > 0).then_some(planned.story_points),
                // An accepted plan is work for today; it can be moved after.
                planned_for: None,
                is_ai_generated: Some(true),
                criteria: Some(planned.acceptance_criteria.clone()),
                files: Some(planned.relevant_files.clone()),
            },
        )?;
        created.push(task);
    }

    for (index, planned) in plan.tasks.iter().enumerate() {
        let ids: Vec<i64> = planned
            .depends_on
            .iter()
            .filter_map(|position| created.get((*position as usize).checked_sub(1)?))
            .map(|task| task.id)
            .collect();
        if !ids.is_empty() {
            replace_dependencies(conn, created[index].id, &ids)?;
        }
    }

    set_goal_status(conn, goal.id, GoalStatus::Accepted)?;
    list_tasks(conn, goal.project_id)
}

/// Finished tasks since a moment, newest first.
///
/// The day each one belongs to is worked out where it is drawn, because "which
/// day" is a question about the reader's clock rather than about UTC.
pub fn completed_since(conn: &Connection, project_id: i64, since: i64) -> Result<Vec<CompletedTask>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, category, story_points, completed_at
         FROM tasks
         WHERE project_id = ?1 AND status = 'completed' AND completed_at IS NOT NULL
           AND completed_at >= ?2
         ORDER BY completed_at DESC",
    )?;
    let rows = stmt.query_map(params![project_id, since], |row| {
        Ok(CompletedTask {
            id: row.get(0)?,
            title: row.get(1)?,
            category: row.get(2)?,
            story_points: row.get(3)?,
            completed_at: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ----------------------------------------------------------- verifications

/// Keeps the verdict so a task's last review survives a restart.
pub fn save_verification(
    conn: &Connection,
    task_id: i64,
    result: &crate::verify::Verification,
) -> Result<()> {
    conn.execute(
        "INSERT INTO task_verifications
           (task_id, created_at, satisfied_count, total_count, summary, raw_result)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            task_id,
            now(),
            result.satisfied() as i64,
            result.total() as i64,
            result.summary,
            serde_json::to_string(result)?,
        ],
    )?;
    Ok(())
}

// ------------------------------------------------------------------ stats

/// A small read on how the work is actually going.
#[derive(serde::Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Focus recorded since local midnight.
    pub focus_today_seconds: i64,
    pub tasks_done_week: i64,
    pub sessions_week: i64,
    /// Mean length of a finished session over the same week.
    pub average_session_seconds: i64,
}

pub fn stats(conn: &Connection) -> Result<Stats> {
    // The boundaries are computed by SQLite so "today" means the developer's
    // today, not UTC's.
    let single = |sql: &str| -> Result<i64> {
        Ok(conn.query_row(sql, [], |row| row.get::<_, Option<i64>>(0))?.unwrap_or(0))
    };

    Ok(Stats {
        focus_today_seconds: single(
            "SELECT COALESCE(SUM(elapsed_seconds), 0) FROM focus_sessions
             WHERE started_at >= CAST(strftime('%s', 'now', 'start of day', 'localtime') AS INTEGER)",
        )?,
        tasks_done_week: single(
            "SELECT COUNT(*) FROM tasks
             WHERE completed_at IS NOT NULL
               AND completed_at >= CAST(strftime('%s', 'now', '-7 days') AS INTEGER)",
        )?,
        sessions_week: single(
            "SELECT COUNT(*) FROM focus_sessions
             WHERE started_at >= CAST(strftime('%s', 'now', '-7 days') AS INTEGER)",
        )?,
        average_session_seconds: single(
            "SELECT CAST(COALESCE(AVG(elapsed_seconds), 0) AS INTEGER) FROM focus_sessions
             WHERE ended_at IS NOT NULL
               AND started_at >= CAST(strftime('%s', 'now', '-7 days') AS INTEGER)",
        )?,
    })
}

// ------------------------------------------------------------ window state

/// The desktop window's geometry, kept out of `Settings` because it is
/// bookkeeping rather than a preference the developer sets.
pub fn window_state(conn: &Connection) -> Result<crate::desktop::WindowState> {
    let number = |key: &str, fallback: f64| -> f64 {
        read_setting(conn, key)
            .ok()
            .flatten()
            .and_then(|raw| raw.parse::<f64>().ok())
            .unwrap_or(fallback)
    };
    let position = match (
        read_setting(conn, "window_x")?.and_then(|raw| raw.parse::<f64>().ok()),
        read_setting(conn, "window_y")?.and_then(|raw| raw.parse::<f64>().ok()),
    ) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    Ok(crate::desktop::WindowState {
        open: read_setting(conn, "window_open")?.as_deref() == Some("1"),
        width: number("window_w", 940.0),
        height: number("window_h", 640.0),
        position,
    })
}

pub fn save_window_state(
    conn: &Connection,
    state: &crate::desktop::WindowState,
) -> Result<()> {
    write_setting(conn, "window_open", if state.open { "1" } else { "0" })?;
    write_setting(conn, "window_w", &state.width.round().to_string())?;
    write_setting(conn, "window_h", &state.height.round().to_string())?;
    if let Some((x, y)) = state.position {
        write_setting(conn, "window_x", &x.round().to_string())?;
        write_setting(conn, "window_y", &y.round().to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------- settings

fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

fn write_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_settings(conn: &Connection) -> Result<Settings> {
    let mut settings = Settings::default();
    if let Some(raw) = read_setting(conn, "focus_minutes")? {
        if let Ok(value) = raw.parse::<i64>() {
            settings.focus_minutes = value.clamp(1, 240);
        }
    }
    if let Some(raw) = read_setting(conn, "show_timer_in_menu_bar")? {
        settings.show_timer_in_menu_bar = raw == "1";
    }
    if let Some(raw) = read_setting(conn, "hide_popup_on_blur")? {
        settings.hide_popup_on_blur = raw == "1";
    }
    // A stale id (project since deleted) reads back as "no active project".
    if let Some(raw) = read_setting(conn, "agent")? {
        settings.agent = crate::agent::Agent::parse_lenient(&raw);
    }
    if let Some(raw) = read_setting(conn, "theme")? {
        settings.theme = Theme::parse_lenient(&raw);
    }
    if let Some(raw) = read_setting(conn, "notifications")? {
        settings.notifications = raw == "1";
    }
    if let Some(raw) = read_setting(conn, "sounds")? {
        settings.sounds = raw == "1";
    }
    if let Some(raw) = read_setting(conn, "launch_at_login")? {
        settings.launch_at_login = raw == "1";
    }
    if let Some(raw) = read_setting(conn, "focus_block_enabled")? {
        settings.focus_block_enabled = raw == "1";
    }
    // An empty list is a real choice — blocking nothing — so it is stored as a
    // written setting and only the absent key falls back to the defaults.
    if let Some(raw) = read_setting(conn, "focus_block_sites")? {
        settings.focus_block_sites = crate::block::parse_sites(&raw);
    }
    if let Some(raw) = read_setting(conn, "onboarded")? {
        settings.onboarded = raw == "1";
    }
    settings.codex_path = read_setting(conn, "codex_path")?.filter(|v| !v.trim().is_empty());
    settings.claude_path = read_setting(conn, "claude_path")?.filter(|v| !v.trim().is_empty());
    settings.claude_model = read_setting(conn, "claude_model")?.filter(|v| !v.trim().is_empty());
    settings.codex_model = read_setting(conn, "codex_model")?.filter(|v| !v.trim().is_empty());
    settings.slack_connected = slack_token(conn)?.is_some();
    settings.active_project_id = match read_setting(conn, "active_project_id")? {
        Some(raw) => raw
            .parse::<i64>()
            .ok()
            .filter(|id| find_project(conn, *id).map(|p| p.is_some()).unwrap_or(false)),
        None => None,
    };
    Ok(settings)
}

/// The Slack token, kept out of `Settings` so it is never serialised to the
/// interface along with everything else.
pub fn slack_token(conn: &Connection) -> Result<Option<String>> {
    Ok(read_setting(conn, "slack_token")?.filter(|token| !token.trim().is_empty()))
}

pub fn update_settings(conn: &Connection, patch: &SettingsPatch) -> Result<Settings> {
    if let Some(minutes) = patch.focus_minutes {
        write_setting(conn, "focus_minutes", &minutes.clamp(1, 240).to_string())?;
    }
    if let Some(show) = patch.show_timer_in_menu_bar {
        write_setting(conn, "show_timer_in_menu_bar", if show { "1" } else { "0" })?;
    }
    if let Some(hide) = patch.hide_popup_on_blur {
        write_setting(conn, "hide_popup_on_blur", if hide { "1" } else { "0" })?;
    }
    if let Some(agent) = patch.agent {
        write_setting(conn, "agent", agent.as_str())?;
    }
    if let Some(theme) = patch.theme {
        write_setting(conn, "theme", theme.as_str())?;
    }
    if let Some(on) = patch.notifications {
        write_setting(conn, "notifications", if on { "1" } else { "0" })?;
    }
    if let Some(on) = patch.sounds {
        write_setting(conn, "sounds", if on { "1" } else { "0" })?;
    }
    if let Some(on) = patch.launch_at_login {
        write_setting(conn, "launch_at_login", if on { "1" } else { "0" })?;
    }
    if let Some(on) = patch.onboarded {
        write_setting(conn, "onboarded", if on { "1" } else { "0" })?;
    }
    if let Some(on) = patch.focus_block_enabled {
        write_setting(conn, "focus_block_enabled", if on { "1" } else { "0" })?;
    }
    if let Some(raw) = &patch.focus_block_sites {
        write_setting(conn, "focus_block_sites", &crate::block::parse_sites(raw).join("\n"))?;
    }
    for (key, value) in [
        ("codex_path", &patch.codex_path),
        ("codex_model", &patch.codex_model),
        ("claude_path", &patch.claude_path),
        ("claude_model", &patch.claude_model),
        ("slack_token", &patch.slack_token),
    ] {
        match value {
            Some(Some(text)) if !text.trim().is_empty() => {
                write_setting(conn, key, text.trim())?;
            }
            Some(_) => {
                conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
            }
            None => {}
        }
    }
    if let Some(active) = patch.active_project_id {
        match active {
            Some(id) => {
                write_setting(conn, "active_project_id", &id.to_string())?;
                touch_project(conn, id)?;
            }
            None => {
                conn.execute("DELETE FROM settings WHERE key = 'active_project_id'", [])?;
            }
        }
    }
    get_settings(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate(&conn).expect("migrate");
        conn
    }

    fn a_project(conn: &Connection) -> Project {
        upsert_project(conn, "/tmp/demo", "demo", true, Some("main")).unwrap()
    }

    fn a_task(conn: &Connection, project_id: i64, title: &str) -> Task {
        create_task(
            conn,
            &NewTask {
                project_id,
                title: title.to_string(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn a_fresh_database_starts_with_task_and_bug() {
        let conn = memory_db();
        let labels: Vec<String> = list_categories(&conn)
            .unwrap()
            .into_iter()
            .map(|category| category.label)
            .collect();
        assert_eq!(labels, vec!["Task", "Bug"]);
    }

    #[test]
    fn the_old_fixed_categories_collapse_onto_task_but_bugs_stay_bugs() {
        // A database from before categories were rows, with the enum's values.
        let before = MIGRATIONS
            .iter()
            .position(|migration| *migration == ADD_CATEGORIES)
            .expect("the categories migration is in the list");
        let conn = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..before] {
            conn.execute_batch(migration).unwrap();
        }
        conn.execute(
            "INSERT INTO projects (path, name, is_git, created_at, last_opened_at)
             VALUES ('/tmp/old', 'old', 1, 1, 1)",
            [],
        )
        .unwrap();
        for (title, category) in [("A", "frontend"), ("B", "devops"), ("C", "bug")] {
            conn.execute(
                "INSERT INTO tasks (project_id, title, category, created_at, updated_at)
                 VALUES (1, ?1, ?2, 1, 1)",
                params![title, category],
            )
            .unwrap();
        }
        conn.pragma_update(None, "user_version", before as i64).unwrap();

        migrate(&conn).unwrap();

        let mut stmt = conn.prepare("SELECT category FROM tasks ORDER BY id").unwrap();
        let categories: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(categories, vec!["task", "task", "bug"]);
    }

    #[test]
    fn a_new_category_is_slugged_and_cannot_be_added_twice() {
        let conn = memory_db();
        let created = create_category(&conn, "  Design Review ", "#123456").unwrap();
        assert_eq!(created.slug, "design-review");
        assert_eq!(created.label, "Design Review");
        assert!(create_category(&conn, "design review", "#123456").is_err());
    }

    #[test]
    fn renaming_a_category_keeps_the_tasks_filed_under_it() {
        let conn = memory_db();
        let project = a_project(&conn);
        let design = create_category(&conn, "Design", "#123456").unwrap();
        let task = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "Sketch the flow".into(),
                category: Some("design".into()),
                ..Default::default()
            },
        )
        .unwrap();

        update_category(
            &conn,
            design.id,
            &CategoryPatch {
                label: Some("Product design".into()),
                color: None,
            },
        )
        .unwrap();

        assert_eq!(get_task(&conn, task.id).unwrap().category, "design");
    }

    #[test]
    fn deleting_a_category_moves_its_tasks_to_the_first_one() {
        let conn = memory_db();
        let project = a_project(&conn);
        let chore = create_category(&conn, "Chore", "#123456").unwrap();
        let task = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "Bump deps".into(),
                category: Some("chore".into()),
                ..Default::default()
            },
        )
        .unwrap();

        delete_category(&conn, chore.id).unwrap();

        assert_eq!(get_task(&conn, task.id).unwrap().category, "task");
    }

    #[test]
    fn the_last_category_cannot_be_deleted() {
        let conn = memory_db();
        let categories = list_categories(&conn).unwrap();
        for category in &categories[1..] {
            delete_category(&conn, category.id).unwrap();
        }
        assert!(delete_category(&conn, categories[0].id).is_err());
    }

    #[test]
    fn a_task_filed_under_a_category_that_is_gone_falls_back() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "Was planned as devops".into(),
                category: Some("devops".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(task.category, "task");
    }

    #[test]
    fn the_history_holds_only_finished_work_from_the_window_asked_for() {
        let conn = memory_db();
        let project = a_project(&conn);
        let old = a_task(&conn, project.id, "Long ago");
        let recent = a_task(&conn, project.id, "Yesterday");
        let open = a_task(&conn, project.id, "Still going");
        for id in [old.id, recent.id] {
            update_task(
                &conn,
                id,
                &TaskPatch {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        // Push one of them outside the window.
        conn.execute(
            "UPDATE tasks SET completed_at = ?2 WHERE id = ?1",
            params![old.id, now() - 400 * 86_400],
        )
        .unwrap();

        let history = completed_since(&conn, project.id, now() - 365 * 86_400).unwrap();
        let titles: Vec<String> = history.into_iter().map(|task| task.title).collect();
        assert_eq!(titles, vec!["Yesterday"]);
        assert!(get_task(&conn, open.id).unwrap().completed_at.is_none());
    }

    #[test]
    fn a_new_task_lands_on_today_unless_a_day_is_given() {
        let conn = memory_db();
        let project = a_project(&conn);
        let today = today(&conn).unwrap();
        assert_eq!(a_task(&conn, project.id, "Now").planned_for, today);

        let planned = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "Tomorrow".into(),
                planned_for: Some("2030-01-02".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(planned.planned_for, "2030-01-02");
    }

    #[test]
    fn a_day_that_is_not_a_date_falls_back_rather_than_being_stored() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "Whenever".into(),
                planned_for: Some("next tuesday".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(task.planned_for, today(&conn).unwrap());
    }

    #[test]
    fn a_task_can_be_moved_to_another_day() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "Move me");
        let moved = update_task(
            &conn,
            task.id,
            &TaskPatch {
                planned_for: Some("2030-03-04".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(moved.planned_for, "2030-03-04");
    }

    #[test]
    fn tasks_from_before_the_upgrade_take_the_day_they_were_made() {
        let before = MIGRATIONS
            .iter()
            .position(|migration| *migration == ADD_PLANNED_FOR)
            .expect("the planned-for migration is in the list");
        let conn = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..before] {
            conn.execute_batch(migration).unwrap();
        }
        conn.execute(
            "INSERT INTO projects (path, name, is_git, created_at, last_opened_at)
             VALUES ('/tmp/old', 'old', 1, 1, 1)",
            [],
        )
        .unwrap();
        // Made at noon UTC on a known day.
        conn.execute(
            "INSERT INTO tasks (project_id, title, created_at, updated_at)
             VALUES (1, 'Older', 1755000000, 1755000000)",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", before as i64).unwrap();

        migrate(&conn).unwrap();

        let day: String = conn
            .query_row("SELECT planned_for FROM tasks WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        let expected: String = conn
            .query_row(
                "SELECT date(1755000000, 'unixepoch', 'localtime')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(day, expected);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = memory_db();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn an_older_database_upgrades_instead_of_being_rebuilt() {
        // Stand up a database at version 1 and carry a row across the upgrade.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1i64).unwrap();
        conn.execute(
            "INSERT INTO projects (path, name, is_git, created_at, last_opened_at)
             VALUES ('/tmp/old', 'old', 1, 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO goals (project_id, title, prompt, status, created_at)
             VALUES (1, 'Old goal', 'do a thing', 'draft', 1)",
            [],
        )
        .unwrap();

        migrate(&conn).expect("upgrade should apply cleanly");

        let goal = get_goal(&conn, 1).unwrap();
        assert_eq!(goal.title, "Old goal");
        assert_eq!(goal.plan_is_none(), true);
    }

    #[test]
    fn a_goal_title_is_taken_from_the_first_line_of_the_prompt() {
        assert_eq!(
            title_from_prompt("Allow customers to download invoices as PDF. Also email them."),
            "Allow customers to download invoices as PDF"
        );
        assert_eq!(title_from_prompt("   "), "Untitled goal");
    }

    #[test]
    fn accepting_a_plan_creates_tasks_and_wires_their_dependencies() {
        let conn = memory_db();
        let project = a_project(&conn);
        let goal = create_goal(&conn, project.id, "Download invoices as PDF", Default::default()).unwrap();

        let plan = crate::plan::Plan {
            summary: "…".into(),
            existing: vec![],
            missing: vec![],
            tasks: vec![
                crate::plan::PlannedTask {
                    title: "PDF service".into(),
                    category: "backend".into(),
                    acceptance_criteria: vec!["Renders a PDF".into()],
                    relevant_files: vec!["src/pdf.ts".into()],
                    estimate_minutes: 45,
                    ..Default::default()
                },
                crate::plan::PlannedTask {
                    title: "Download endpoint".into(),
                    category: "backend".into(),
                    depends_on: vec![1],
                    ..Default::default()
                },
            ],
        };
        let goal = set_goal_plan(&conn, goal.id, "thread-1", &plan).unwrap();

        let tasks = create_tasks_from_plan(&conn, &goal).unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].is_ai_generated);
        assert_eq!(tasks[0].criteria.len(), 1);
        assert_eq!(tasks[0].files, vec!["src/pdf.ts"]);
        // The plan's 1-based position became the first task's real id.
        assert_eq!(tasks[1].depends_on, vec![tasks[0].id]);
        assert_eq!(get_goal(&conn, goal.id).unwrap().status, GoalStatus::Accepted);
    }

    #[test]
    fn a_goal_awaiting_a_decision_outranks_a_newer_finished_one() {
        let conn = memory_db();
        let project = a_project(&conn);
        let ready = create_goal(&conn, project.id, "Needs a decision", Default::default()).unwrap();
        set_goal_status(&conn, ready.id, GoalStatus::Ready).unwrap();
        let done = create_goal(&conn, project.id, "Already accepted", Default::default()).unwrap();
        set_goal_status(&conn, done.id, GoalStatus::Accepted).unwrap();

        let latest = latest_goal(&conn, project.id).unwrap().unwrap();
        assert_eq!(latest.id, ready.id);
    }

    #[test]
    fn re_adding_a_path_updates_rather_than_duplicates() {
        let conn = memory_db();
        let first = a_project(&conn);
        let second = upsert_project(&conn, "/tmp/demo", "demo", true, Some("feature/x")).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.branch.as_deref(), Some("feature/x"));
        assert_eq!(list_projects(&conn).unwrap().len(), 1);
    }

    #[test]
    fn new_tasks_append_to_the_end_of_the_list() {
        let conn = memory_db();
        let project = a_project(&conn);
        let first = a_task(&conn, project.id, "first");
        let second = a_task(&conn, project.id, "second");
        assert_eq!((first.position, second.position), (0, 1));
    }

    #[test]
    fn a_blank_title_is_rejected() {
        let conn = memory_db();
        let project = a_project(&conn);
        let result = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "   ".to_string(),
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn rewriting_criteria_keeps_the_state_of_unchanged_lines() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = create_task(
            &conn,
            &NewTask {
                project_id: project.id,
                title: "PDF service".to_string(),
                criteria: Some(vec!["Generate PDF".into(), "Handle errors".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        set_criterion_met(&conn, task.criteria[0].id, true).unwrap();

        let updated = update_task(
            &conn,
            task.id,
            &TaskPatch {
                criteria: Some(vec![
                    "Generate PDF".into(),
                    "Handle errors".into(),
                    "Write tests".into(),
                ]),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(updated.criteria[0].is_met, "unchanged line keeps its tick");
        assert!(!updated.criteria[1].is_met);
        assert!(!updated.criteria[2].is_met);
    }

    #[test]
    fn completing_stamps_the_time_and_reopening_clears_it() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "ship it");

        let done = update_task(
            &conn,
            task.id,
            &TaskPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(done.completed_at.is_some());

        let reopened = update_task(
            &conn,
            task.id,
            &TaskPatch {
                status: Some(TaskStatus::Ready),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(reopened.completed_at, None);
    }

    #[test]
    fn reordering_rewrites_positions_in_the_given_order() {
        let conn = memory_db();
        let project = a_project(&conn);
        let first = a_task(&conn, project.id, "first");
        let second = a_task(&conn, project.id, "second");
        let third = a_task(&conn, project.id, "third");

        reorder_tasks(&conn, project.id, &[third.id, first.id, second.id]).unwrap();

        let titles: Vec<String> = list_tasks(&conn, project.id)
            .unwrap()
            .into_iter()
            .map(|task| task.title)
            .collect();
        assert_eq!(titles, vec!["third", "first", "second"]);
    }

    #[test]
    fn a_task_cannot_depend_on_itself() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "loop");
        let result = update_task(
            &conn,
            task.id,
            &TaskPatch {
                depends_on: Some(vec![task.id]),
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn deleting_a_project_takes_its_tasks_with_it() {
        let conn = memory_db();
        let project = a_project(&conn);
        a_task(&conn, project.id, "orphan");
        delete_project(&conn, project.id).unwrap();
        assert!(list_tasks(&conn, project.id).unwrap().is_empty());
    }

    #[test]
    fn a_task_thread_is_kept_per_agent_and_updated_in_place() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "PDF service");

        save_task_thread(&conn, task.id, crate::agent::Agent::Codex, "codex-1", "running").unwrap();
        save_task_thread(&conn, task.id, crate::agent::Agent::Codex, "codex-1", "finished").unwrap();
        save_task_thread(&conn, task.id, crate::agent::Agent::ClaudeCode, "claude-1", "running")
            .unwrap();

        assert_eq!(
            task_thread(&conn, task.id, crate::agent::Agent::Codex).unwrap(),
            Some("codex-1".to_string())
        );
        assert_eq!(
            task_thread(&conn, task.id, crate::agent::Agent::ClaudeCode).unwrap(),
            Some("claude-1".to_string())
        );
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM codex_threads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 2, "one row per agent, updated rather than appended");
    }

    #[test]
    fn a_task_with_no_run_yet_has_no_thread() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "PDF service");
        assert_eq!(
            task_thread(&conn, task.id, crate::agent::Agent::Codex).unwrap(),
            None
        );
    }

    #[test]
    fn focus_time_and_session_count_accumulate_together() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "deep work");

        let first = open_focus_session(&conn, task.id, 1500).unwrap();
        close_focus_session(&conn, first, 900, false).unwrap();
        let second = open_focus_session(&conn, task.id, 1500).unwrap();
        close_focus_session(&conn, second, 600, true).unwrap();

        let task = get_task(&conn, task.id).unwrap();
        assert_eq!(task.focus_seconds, 1500);
        assert_eq!(task.focus_sessions, 2);
    }

    #[test]
    fn a_task_never_focused_reports_no_sessions() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "untouched");
        let task = get_task(&conn, task.id).unwrap();
        assert_eq!((task.focus_seconds, task.focus_sessions), (0, 0));
    }

    #[test]
    fn stats_are_zero_on_a_fresh_database() {
        let conn = memory_db();
        let stats = stats(&conn).unwrap();
        assert_eq!(stats.focus_today_seconds, 0);
        assert_eq!(stats.tasks_done_week, 0);
        assert_eq!(stats.sessions_week, 0);
        assert_eq!(stats.average_session_seconds, 0);
    }

    #[test]
    fn stats_count_todays_focus_and_this_weeks_work() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "deep work");

        let first = open_focus_session(&conn, task.id, 1500).unwrap();
        close_focus_session(&conn, first, 600, true).unwrap();
        let second = open_focus_session(&conn, task.id, 1500).unwrap();
        close_focus_session(&conn, second, 1200, true).unwrap();
        update_task(
            &conn,
            task.id,
            &TaskPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();

        let stats = stats(&conn).unwrap();
        assert_eq!(stats.focus_today_seconds, 1800, "both sessions were today");
        assert_eq!(stats.sessions_week, 2);
        assert_eq!(stats.tasks_done_week, 1);
        assert_eq!(stats.average_session_seconds, 900);
    }

    #[test]
    fn work_older_than_the_window_is_left_out() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "old work");
        let old = open_focus_session(&conn, task.id, 1500).unwrap();
        close_focus_session(&conn, old, 600, true).unwrap();
        // Backdate it well past the week.
        conn.execute(
            "UPDATE focus_sessions SET started_at = started_at - (30 * 86400) WHERE id = ?1",
            [old],
        )
        .unwrap();

        let stats = stats(&conn).unwrap();
        assert_eq!(stats.sessions_week, 0);
        assert_eq!(stats.focus_today_seconds, 0);
    }

    #[test]
    fn only_the_restored_session_is_left_open() {
        let conn = memory_db();
        let project = a_project(&conn);
        let task = a_task(&conn, project.id, "interrupted");
        let stale = open_focus_session(&conn, task.id, 1500).unwrap();
        let current = open_focus_session(&conn, task.id, 1500).unwrap();

        let open = latest_open_focus_session(&conn).unwrap().unwrap();
        assert_eq!(open.0, current);

        close_stale_focus_sessions(&conn, Some(current)).unwrap();
        let still_open: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM focus_sessions WHERE ended_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(still_open, 1);
        assert_ne!(open.0, stale);
    }

    #[test]
    fn window_state_round_trips_and_defaults_to_closed() {
        let conn = memory_db();
        let fresh = window_state(&conn).unwrap();
        assert!(!fresh.open);
        assert_eq!((fresh.width, fresh.height), (940.0, 640.0));
        assert_eq!(fresh.position, None);

        save_window_state(
            &conn,
            &crate::desktop::WindowState {
                open: true,
                width: 1200.0,
                height: 800.0,
                position: Some((40.0, 60.0)),
            },
        )
        .unwrap();

        let saved = window_state(&conn).unwrap();
        assert!(saved.open);
        assert_eq!((saved.width, saved.height), (1200.0, 800.0));
        assert_eq!(saved.position, Some((40.0, 60.0)));
    }

    #[test]
    fn the_theme_round_trips_and_defaults_to_light() {
        let conn = memory_db();
        assert_eq!(get_settings(&conn).unwrap().theme, Theme::Light);
        let saved = update_settings(
            &conn,
            &SettingsPatch {
                theme: Some(Theme::GithubDark),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saved.theme, Theme::GithubDark);
        assert_eq!(get_settings(&conn).unwrap().theme, Theme::GithubDark);
    }

    #[test]
    fn an_unknown_stored_theme_falls_back_rather_than_failing() {
        let conn = memory_db();
        write_setting(&conn, "theme", "solarized-neon").unwrap();
        assert_eq!(get_settings(&conn).unwrap().theme, Theme::Light);
    }

    #[test]
    fn settings_round_trip_and_clamp_out_of_range_values() {
        let conn = memory_db();
        let saved = update_settings(
            &conn,
            &SettingsPatch {
                focus_minutes: Some(9_000),
                show_timer_in_menu_bar: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saved.focus_minutes, 240);
        assert!(!saved.show_timer_in_menu_bar);
        assert!(saved.hide_popup_on_blur, "untouched keys keep their default");
    }

    #[test]
    fn an_active_project_that_was_deleted_reads_back_as_none() {
        let conn = memory_db();
        let project = a_project(&conn);
        update_settings(
            &conn,
            &SettingsPatch {
                active_project_id: Some(Some(project.id)),
                ..Default::default()
            },
        )
        .unwrap();
        delete_project(&conn, project.id).unwrap();
        assert_eq!(get_settings(&conn).unwrap().active_project_id, None);
    }
}
