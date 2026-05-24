use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::openhuman::config::Config;

use super::types::{AiConversation, AiMessage, ProviderUsageSummary, UserProvider};

// ─── table bootstrap ────────────────────────────────────────────────────────

pub fn ensure_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;

         CREATE TABLE IF NOT EXISTS ai_os_providers (
             id            TEXT PRIMARY KEY,
             name          TEXT NOT NULL,
             kind          TEXT NOT NULL,
             base_url      TEXT NOT NULL,
             api_key       TEXT,
             default_model TEXT NOT NULL,
             enabled       INTEGER NOT NULL DEFAULT 1,
             created_at    INTEGER NOT NULL,
             updated_at    INTEGER NOT NULL,
             vps_url       TEXT
         );

         CREATE TABLE IF NOT EXISTS ai_os_conversations (
             id          TEXT PRIMARY KEY,
             title       TEXT NOT NULL,
             provider_id TEXT NOT NULL,
             model       TEXT NOT NULL,
             created_at  INTEGER NOT NULL,
             updated_at  INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS ai_os_messages (
             id              TEXT PRIMARY KEY,
             conversation_id TEXT NOT NULL,
             role            TEXT NOT NULL,
             content         TEXT NOT NULL,
             input_tokens    INTEGER NOT NULL DEFAULT 0,
             output_tokens   INTEGER NOT NULL DEFAULT 0,
             cost_usd        REAL NOT NULL DEFAULT 0.0,
             created_at      INTEGER NOT NULL
         );

         CREATE INDEX IF NOT EXISTS idx_ai_os_conversations_provider
             ON ai_os_conversations(provider_id);
         CREATE INDEX IF NOT EXISTS idx_ai_os_messages_conversation
             ON ai_os_messages(conversation_id);",
    )
}

// ─── provider ops ───────────────────────────────────────────────────────────

pub fn provider_upsert(conn: &Connection, p: &UserProvider) -> rusqlite::Result<()> {
    let kind = serde_json::to_string(&p.kind).unwrap_or_else(|_| "\"open_ai_compatible\"".into());
    // strip surrounding quotes that serde adds for string enums
    let kind = kind.trim_matches('"').to_string();
    conn.execute(
        "INSERT INTO ai_os_providers
             (id, name, kind, base_url, api_key, default_model, enabled, created_at, updated_at,
              vps_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
             name          = excluded.name,
             kind          = excluded.kind,
             base_url      = excluded.base_url,
             api_key       = excluded.api_key,
             default_model = excluded.default_model,
             enabled       = excluded.enabled,
             updated_at    = excluded.updated_at,
             vps_url       = excluded.vps_url",
        params![
            p.id,
            p.name,
            kind,
            p.base_url,
            p.api_key,
            p.default_model,
            p.enabled as i64,
            p.created_at,
            p.updated_at,
            p.vps_url,
        ],
    )?;
    Ok(())
}

pub fn provider_get(conn: &Connection, id: &str) -> rusqlite::Result<Option<UserProvider>> {
    conn.query_row(
        "SELECT id, name, kind, base_url, api_key, default_model, enabled, created_at, updated_at,
                vps_url
         FROM ai_os_providers WHERE id = ?1",
        params![id],
        row_to_provider,
    )
    .optional()
}

pub fn provider_list(conn: &Connection) -> rusqlite::Result<Vec<UserProvider>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, base_url, api_key, default_model, enabled, created_at, updated_at,
                vps_url
         FROM ai_os_providers ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_provider)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn provider_delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM ai_os_providers WHERE id = ?1", params![id])?;
    Ok(())
}

fn row_to_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserProvider> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let kind_str: String = row.get(2)?;
    let base_url: String = row.get(3)?;
    let api_key: Option<String> = row.get(4)?;
    let default_model: String = row.get(5)?;
    let enabled: i64 = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let updated_at: i64 = row.get(8)?;
    let vps_url: Option<String> = row.get(9)?;

    // Parse kind from stored string (snake_case without quotes)
    let kind = serde_json::from_str(&format!("\"{}\"", kind_str))
        .unwrap_or(crate::openhuman::ai_os::types::ProviderKind::OpenAiCompatible);

    Ok(UserProvider {
        id,
        name,
        kind,
        base_url,
        api_key,
        default_model,
        enabled: enabled != 0,
        created_at,
        updated_at,
        vps_url,
    })
}

// ─── conversation ops ────────────────────────────────────────────────────────

pub fn conversation_upsert(conn: &Connection, c: &AiConversation) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO ai_os_conversations (id, title, provider_id, model, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
             title      = excluded.title,
             provider_id = excluded.provider_id,
             model      = excluded.model,
             updated_at = excluded.updated_at",
        params![
            c.id,
            c.title,
            c.provider_id,
            c.model,
            c.created_at,
            c.updated_at
        ],
    )?;
    Ok(())
}

pub fn conversation_list(
    conn: &Connection,
    provider_id: Option<&str>,
) -> rusqlite::Result<Vec<AiConversation>> {
    if let Some(pid) = provider_id {
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_id, model, created_at, updated_at
             FROM ai_os_conversations WHERE provider_id = ?1 ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![pid], row_to_conversation)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_id, model, created_at, updated_at
             FROM ai_os_conversations ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map([], row_to_conversation)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

pub fn conversation_get(conn: &Connection, id: &str) -> rusqlite::Result<Option<AiConversation>> {
    conn.query_row(
        "SELECT id, title, provider_id, model, created_at, updated_at
         FROM ai_os_conversations WHERE id = ?1",
        params![id],
        row_to_conversation,
    )
    .optional()
}

pub fn conversation_delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM ai_os_messages WHERE conversation_id = ?1",
        params![id],
    )?;
    conn.execute("DELETE FROM ai_os_conversations WHERE id = ?1", params![id])?;
    Ok(())
}

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiConversation> {
    Ok(AiConversation {
        id: row.get(0)?,
        title: row.get(1)?,
        provider_id: row.get(2)?,
        model: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

// ─── message ops ─────────────────────────────────────────────────────────────

pub fn message_insert(conn: &Connection, m: &AiMessage) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO ai_os_messages
             (id, conversation_id, role, content, input_tokens, output_tokens, cost_usd, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            m.id,
            m.conversation_id,
            m.role,
            m.content,
            m.input_tokens,
            m.output_tokens,
            m.cost_usd,
            m.created_at,
        ],
    )?;
    Ok(())
}

pub fn messages_for_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<Vec<AiMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, input_tokens, output_tokens, cost_usd, created_at
         FROM ai_os_messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], row_to_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiMessage> {
    Ok(AiMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        input_tokens: row.get(4)?,
        output_tokens: row.get(5)?,
        cost_usd: row.get(6)?,
        created_at: row.get(7)?,
    })
}

// ─── usage aggregation ───────────────────────────────────────────────────────

pub fn usage_summary(conn: &Connection, days: i64) -> rusqlite::Result<Vec<ProviderUsageSummary>> {
    let cutoff = chrono::Utc::now().timestamp() - days * 86_400;
    let mut stmt = conn.prepare(
        "SELECT
             p.id,
             p.name,
             COALESCE(SUM(m.input_tokens), 0)  AS total_input,
             COALESCE(SUM(m.output_tokens), 0) AS total_output,
             COALESCE(SUM(m.cost_usd), 0.0)    AS total_cost,
             COUNT(m.id)                        AS req_count
         FROM ai_os_providers p
         LEFT JOIN ai_os_conversations c ON c.provider_id = p.id
         LEFT JOIN ai_os_messages m ON m.conversation_id = c.id AND m.created_at >= ?1
         GROUP BY p.id, p.name
         ORDER BY total_cost DESC",
    )?;
    let rows = stmt
        .query_map(params![cutoff], |row| {
            Ok(ProviderUsageSummary {
                provider_id: row.get(0)?,
                provider_name: row.get(1)?,
                total_input_tokens: row.get(2)?,
                total_output_tokens: row.get(3)?,
                total_cost_usd: row.get(4)?,
                request_count: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// ─── connection helper ────────────────────────────────────────────────────────

pub fn with_connection<T>(config: &Config, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let db_path = config.workspace_dir.join("ai_os").join("ai_os.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "[ai_os] failed to create ai_os directory: {}",
                parent.display()
            )
        })?;
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("[ai_os] failed to open DB: {}", db_path.display()))?;

    ensure_tables(&conn).with_context(|| "[ai_os] failed to initialize schema")?;
    migrate_add_vps_url(&conn).with_context(|| "[ai_os] failed to run vps_url migration")?;

    f(&conn)
}

/// Migration: add `vps_url` column to `ai_os_providers` for existing databases.
///
/// SQLite does not support `ADD COLUMN IF NOT EXISTS`, so we check the
/// current schema via `PRAGMA table_info` first and skip the ALTER if the
/// column is already present.
fn migrate_add_vps_url(conn: &Connection) -> rusqlite::Result<()> {
    let column_exists = conn
        .prepare("PRAGMA table_info(ai_os_providers)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.map(|n| n == "vps_url").unwrap_or(false));

    if !column_exists {
        tracing::debug!("[ai_os][store] migrating ai_os_providers: adding vps_url column");
        conn.execute_batch(
            "ALTER TABLE ai_os_providers ADD COLUMN vps_url TEXT;",
        )?;
    }
    Ok(())
}
