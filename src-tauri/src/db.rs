// SQLite layer — clients + reports.
use crate::{ClientInput, ReportSummary};
use anyhow::Result;
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

#[derive(Debug, Serialize, Clone)]
pub struct StrategicHealthReport {
    pub id: i64,
    pub generated_at: String,
    pub content: String,
}

fn db_path() -> std::path::PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let new_dir = base.join("GrowBox");
    let old_dir = base.join("ReachOptimizer");
    if old_dir.exists() && !new_dir.exists() {
        let _ = std::fs::rename(&old_dir, &new_dir);
    }
    std::fs::create_dir_all(&new_dir).ok();
    let new_db = new_dir.join("growbox.sqlite");
    let old_db = new_dir.join("reach.sqlite");
    if old_db.exists() && !new_db.exists() {
        let _ = std::fs::rename(&old_db, &new_db);
    }
    new_db
}

pub async fn init() -> Result<SqlitePool> {
    let path = db_path();
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS clients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            tiktok TEXT, instagram TEXT, snapchat TEXT,
            niche TEXT NOT NULL,
            countries TEXT NOT NULL,
            plan_days INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"#,
    ).execute(&pool).await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            client_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY(client_id) REFERENCES clients(id)
        )"#,
    ).execute(&pool).await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS strategic_health_reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"#,
    ).execute(&pool).await?;
    Ok(pool)
}

pub async fn insert_client(pool: &SqlitePool, c: &ClientInput) -> Result<i64> {
    let countries = c.countries.join(",");
    let row = sqlx::query("INSERT INTO clients (name, tiktok, instagram, snapchat, niche, countries, plan_days) VALUES (?,?,?,?,?,?,?)")
        .bind(&c.name).bind(&c.tiktok).bind(&c.instagram).bind(&c.snapchat)
        .bind(&c.niche).bind(&countries).bind(c.plan_days as i64)
        .execute(pool).await?;
    Ok(row.last_insert_rowid())
}

pub async fn save_report(pool: &SqlitePool, client_id: i64, content: &str) -> Result<()> {
    sqlx::query("INSERT INTO reports (client_id, content) VALUES (?,?)")
        .bind(client_id).bind(content).execute(pool).await?;
    Ok(())
}

pub async fn list_clients(pool: &SqlitePool) -> Result<Vec<ReportSummary>> {
    let rows = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, name, created_at FROM clients ORDER BY created_at DESC LIMIT 50",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id, name, created_at)| ReportSummary {
        id, client_name: name, generated_at: created_at,
        status: "ready".into(), content: String::new(),
    }).collect())
}

pub async fn load_latest_report(pool: &SqlitePool, client_id: i64) -> Result<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM reports WHERE client_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0).unwrap_or_default())
}

pub async fn load_recent_report_contents(pool: &SqlitePool, limit: i64) -> Result<Vec<(String, String, String)>> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT c.name, r.created_at, r.content
           FROM reports r
           JOIN clients c ON c.id = r.client_id
           ORDER BY r.created_at DESC
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn save_strategic_health_report(pool: &SqlitePool, content: &str) -> Result<i64> {
    let row = sqlx::query("INSERT INTO strategic_health_reports (content) VALUES (?)")
        .bind(content)
        .execute(pool)
        .await?;
    Ok(row.last_insert_rowid())
}

pub async fn list_strategic_health_reports(pool: &SqlitePool) -> Result<Vec<StrategicHealthReport>> {
    let rows = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, created_at, content FROM strategic_health_reports ORDER BY created_at DESC LIMIT 20",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, generated_at, content)| StrategicHealthReport { id, generated_at, content })
        .collect())
}
