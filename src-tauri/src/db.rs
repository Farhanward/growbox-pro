// SQLite layer — clients + reports.
use crate::{ClientInput, ReportSummary};
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

fn db_path() -> std::path::PathBuf {
    let base = dirs::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("ReachOptimizer");
    std::fs::create_dir_all(&base).ok();
    base.join("reach.sqlite")
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
