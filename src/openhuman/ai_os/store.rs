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
    )?;

    // Migration: add `vps_url` column to `ai_os_providers` for existing databases.
    // SQLite does not support `ADD COLUMN IF NOT EXISTS`, so we check via
    // PRAGMA table_info first and skip the ALTER if the column is already present.
    let column_exists = conn
        .prepare("PRAGMA table_info(ai_os_providers)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.map(|n| n == "vps_url").unwrap_or(false));

    if !column_exists {
        tracing::debug!("[ai_os][store] migrating ai_os_providers: adding vps_url column");
        conn.execute_batch("ALTER TABLE ai_os_providers ADD COLUMN vps_url TEXT;")?;
    }

    // Migration: rename legacy `open_ai_compatible` provider kind to the
    // canonical `openai_compatible` wire token. Idempotent: no-op once
    // rows already use the new value.
    let legacy_kind_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_os_providers WHERE kind = 'open_ai_compatible'",
        [],
        |row| row.get(0),
    )?;
    if legacy_kind_count > 0 {
        tracing::debug!(
            count = legacy_kind_count,
            "[ai_os][store] migrating provider kind: open_ai_compatible -> openai_compatible"
        );
        conn.execute(
            "UPDATE ai_os_providers SET kind = 'openai_compatible' WHERE kind = 'open_ai_compatible'",
            [],
        )?;
    }

    // FTS5 index for message content. Standalone (non-external) virtual
    // table — duplicates content for O(log n) full-text lookups, kept in
    // sync via triggers on `ai_os_messages`.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS ai_os_messages_fts USING fts5(
             content,
             message_id UNINDEXED,
             conversation_id UNINDEXED,
             tokenize = 'porter unicode61'
         );

         CREATE TRIGGER IF NOT EXISTS ai_os_messages_fts_ai
             AFTER INSERT ON ai_os_messages BEGIN
                 INSERT INTO ai_os_messages_fts (content, message_id, conversation_id)
                 VALUES (new.content, new.id, new.conversation_id);
             END;

         CREATE TRIGGER IF NOT EXISTS ai_os_messages_fts_ad
             AFTER DELETE ON ai_os_messages BEGIN
                 DELETE FROM ai_os_messages_fts WHERE message_id = old.id;
             END;

         CREATE TRIGGER IF NOT EXISTS ai_os_messages_fts_au
             AFTER UPDATE ON ai_os_messages BEGIN
                 DELETE FROM ai_os_messages_fts WHERE message_id = old.id;
                 INSERT INTO ai_os_messages_fts (content, message_id, conversation_id)
                 VALUES (new.content, new.id, new.conversation_id);
             END;",
    )?;

    // One-time backfill: if FTS is empty but `ai_os_messages` has rows, populate.
    let fts_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM ai_os_messages_fts", [], |row| {
            row.get(0)
        })?;
    let msg_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM ai_os_messages", [], |row| row.get(0))?;
    if fts_count == 0 && msg_count > 0 {
        tracing::debug!(
            count = msg_count,
            "[ai_os][store] backfilling ai_os_messages_fts from existing messages"
        );
        conn.execute(
            "INSERT INTO ai_os_messages_fts (content, message_id, conversation_id)
             SELECT content, id, conversation_id FROM ai_os_messages",
            [],
        )?;
    }

    Ok(())
}

// ─── provider ops ───────────────────────────────────────────────────────────

pub fn provider_upsert(conn: &Connection, p: &UserProvider) -> rusqlite::Result<()> {
    let kind = serde_json::to_string(&p.kind).unwrap_or_else(|_| "\"openai_compatible\"".into());
    // strip surrounding quotes that serde adds for string enums
    let kind = kind.trim_matches('"').to_string();

    // Encrypt the API key before storing it.  Non-empty keys are always encrypted;
    // absent or empty keys are stored as NULL / empty unchanged.
    let encrypted_api_key: Option<String> = match &p.api_key {
        Some(key) if !key.is_empty() => {
            let enc = super::key::encrypt_api_key(key).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(e))
            })?;
            tracing::debug!("[ai_os][store] encrypted api_key for provider {}", p.id);
            Some(enc)
        }
        other => other.clone(),
    };

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
            encrypted_api_key,
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
    let raw_api_key: Option<String> = row.get(4)?;
    let default_model: String = row.get(5)?;
    let enabled: i64 = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let updated_at: i64 = row.get(8)?;
    let vps_url: Option<String> = row.get(9)?;

    // Decrypt the API key transparently.  Plaintext legacy values are returned
    // as-is by `decrypt_api_key` so existing rows continue to work.
    let api_key: Option<String> = match raw_api_key {
        Some(ref enc) if !enc.is_empty() => {
            let plaintext = super::key::decrypt_api_key(enc).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::<dyn std::error::Error + Send + Sync>::from(e),
                )
            })?;
            tracing::debug!("[ai_os][store] decrypted api_key for provider {}", id);
            Some(plaintext)
        }
        other => other,
    };

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

pub fn conversation_search(
    conn: &rusqlite::Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<Vec<AiConversation>> {
    // Empty / whitespace-only queries return no matches rather than
    // matching everything — `LIKE '%%'` would have, but that's a footgun.
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let title_pattern = format!("%{}%", trimmed);
    let fts_query = escape_fts5_query(trimmed);

    // Title matches with LIKE (titles are short, FTS overhead isn't worth it).
    // Body matches with FTS5 MATCH for index-backed lookup. UNION via OR
    // against an `IN (SELECT …)` keeps the planner happy and avoids a
    // cross-product LEFT JOIN over the whole messages table.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.id, c.title, c.provider_id, c.model, c.created_at, c.updated_at
         FROM ai_os_conversations c
         WHERE c.title LIKE ?1
            OR c.id IN (
                SELECT DISTINCT conversation_id
                FROM ai_os_messages_fts
                WHERE ai_os_messages_fts MATCH ?2
            )
         ORDER BY c.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(
            params![title_pattern, fts_query, limit as i64],
            row_to_conversation,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Escape a free-text user query for safe use as an FTS5 MATCH expression.
///
/// FTS5 query syntax treats `"`, `(`, `)`, `*`, `-`, `+`, `:`, `^`, `NEAR`,
/// `AND`, `OR`, `NOT` as operators or column qualifiers. Wrapping each
/// whitespace-delimited token in double quotes (with inner `"` doubled per
/// FTS5 string literal rules) renders the query as a plain phrase/term
/// search regardless of what the user typed.
fn escape_fts5_query(q: &str) -> String {
    q.split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
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
    // `created_at` is stored as milliseconds since epoch; compute the cutoff
    // in the same unit so the comparison is correct.
    let cutoff = chrono::Utc::now().timestamp_millis() - days * 86_400 * 1_000;
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

    f(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openhuman::ai_os::types::ProviderKind;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();
        conn
    }

    fn make_provider(id: &str, kind: ProviderKind) -> UserProvider {
        UserProvider {
            id: id.into(),
            name: format!("p-{id}"),
            kind,
            base_url: "https://example.com".into(),
            api_key: None,
            default_model: "m".into(),
            enabled: true,
            created_at: 0,
            updated_at: 0,
            vps_url: None,
        }
    }

    fn make_conv(id: &str, provider_id: &str, title: &str) -> AiConversation {
        AiConversation {
            id: id.into(),
            title: title.into(),
            provider_id: provider_id.into(),
            model: "m".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_msg(id: &str, conv_id: &str, content: &str) -> AiMessage {
        AiMessage {
            id: id.into(),
            conversation_id: conv_id.into(),
            role: "user".into(),
            content: content.into(),
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            created_at: 0,
        }
    }

    #[test]
    fn ensure_tables_creates_fts_virtual_table() {
        let conn = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'table' AND name = 'ai_os_messages_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "FTS5 virtual table must be created");
    }

    #[test]
    fn provider_kind_round_trips_through_store() {
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p1", ProviderKind::OpenAiCompatible)).unwrap();
        provider_upsert(&conn, &make_provider("p2", ProviderKind::Anthropic)).unwrap();
        provider_upsert(&conn, &make_provider("p3", ProviderKind::Google)).unwrap();

        let p1 = provider_get(&conn, "p1").unwrap().unwrap();
        let p2 = provider_get(&conn, "p2").unwrap().unwrap();
        let p3 = provider_get(&conn, "p3").unwrap().unwrap();

        assert_eq!(p1.kind, ProviderKind::OpenAiCompatible);
        assert_eq!(p2.kind, ProviderKind::Anthropic);
        assert_eq!(p3.kind, ProviderKind::Google);

        // Stored kind value must be the canonical wire token.
        let raw_kind: String = conn
            .query_row(
                "SELECT kind FROM ai_os_providers WHERE id = 'p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw_kind, "openai_compatible");
    }

    #[test]
    fn legacy_provider_kind_is_migrated() {
        let conn = fresh_conn();

        // Seed a row written by an older build that used the
        // `open_ai_compatible` wire form.
        conn.execute(
            "INSERT INTO ai_os_providers
                (id, name, kind, base_url, default_model, enabled, created_at, updated_at)
             VALUES ('legacy', 'old', 'open_ai_compatible', '', 'm', 1, 0, 0)",
            [],
        )
        .unwrap();

        // Re-run ensure_tables — migration should rewrite the row.
        ensure_tables(&conn).unwrap();

        let kind: String = conn
            .query_row(
                "SELECT kind FROM ai_os_providers WHERE id = 'legacy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "openai_compatible");

        let loaded = provider_get(&conn, "legacy").unwrap().unwrap();
        assert_eq!(loaded.kind, ProviderKind::OpenAiCompatible);
    }

    #[test]
    fn conversation_search_finds_match_by_message_content_via_fts() {
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p", ProviderKind::OpenAiCompatible)).unwrap();
        conversation_upsert(&conn, &make_conv("c1", "p", "Untitled")).unwrap();
        conversation_upsert(&conn, &make_conv("c2", "p", "Untitled")).unwrap();
        message_insert(&conn, &make_msg("m1", "c1", "tell me about rust ownership")).unwrap();
        message_insert(&conn, &make_msg("m2", "c2", "completely unrelated text")).unwrap();

        let hits = conversation_search(&conn, "ownership", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "c1");
    }

    #[test]
    fn conversation_search_finds_match_by_title() {
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p", ProviderKind::OpenAiCompatible)).unwrap();
        conversation_upsert(&conn, &make_conv("c1", "p", "Rust deep dive")).unwrap();
        conversation_upsert(&conn, &make_conv("c2", "p", "Python basics")).unwrap();

        let hits = conversation_search(&conn, "Rust", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "c1");
    }

    #[test]
    fn conversation_search_empty_query_returns_nothing() {
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p", ProviderKind::OpenAiCompatible)).unwrap();
        conversation_upsert(&conn, &make_conv("c1", "p", "anything")).unwrap();

        assert!(conversation_search(&conn, "", 10).unwrap().is_empty());
        assert!(conversation_search(&conn, "   ", 10).unwrap().is_empty());
    }

    #[test]
    fn conversation_search_tolerates_fts_operator_chars() {
        // Inputs containing characters that FTS5 would otherwise interpret
        // as operators (quotes, parens, colons, stars, NEAR, AND, OR, NOT)
        // must not error — escape_fts5_query wraps each token as a literal.
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p", ProviderKind::OpenAiCompatible)).unwrap();
        conversation_upsert(&conn, &make_conv("c1", "p", "t")).unwrap();
        message_insert(&conn, &make_msg("m1", "c1", "alpha beta gamma")).unwrap();

        for q in ["alpha AND beta", "\"hello\"", "foo*bar", "x:y", "(a OR b)"] {
            // Should return successfully, even if no hits.
            let _ = conversation_search(&conn, q, 10).unwrap();
        }
    }

    #[test]
    fn fts_trigger_keeps_index_in_sync_on_delete() {
        let conn = fresh_conn();
        provider_upsert(&conn, &make_provider("p", ProviderKind::OpenAiCompatible)).unwrap();
        conversation_upsert(&conn, &make_conv("c1", "p", "t")).unwrap();
        message_insert(&conn, &make_msg("m1", "c1", "unique_phrase_for_search")).unwrap();

        assert_eq!(
            conversation_search(&conn, "unique_phrase_for_search", 10)
                .unwrap()
                .len(),
            1
        );

        conversation_delete(&conn, "c1").unwrap();

        assert_eq!(
            conversation_search(&conn, "unique_phrase_for_search", 10)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn fts_backfill_populates_from_pre_existing_messages() {
        // Simulate a DB written by an older build that lacked FTS5: create
        // the base tables manually, insert a message, then run
        // `ensure_tables` and confirm the backfill copies the row.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE ai_os_providers (
                 id TEXT PRIMARY KEY, name TEXT, kind TEXT, base_url TEXT,
                 api_key TEXT, default_model TEXT, enabled INTEGER,
                 created_at INTEGER, updated_at INTEGER
             );
             CREATE TABLE ai_os_conversations (
                 id TEXT PRIMARY KEY, title TEXT, provider_id TEXT, model TEXT,
                 created_at INTEGER, updated_at INTEGER
             );
             CREATE TABLE ai_os_messages (
                 id TEXT PRIMARY KEY, conversation_id TEXT, role TEXT,
                 content TEXT, input_tokens INTEGER, output_tokens INTEGER,
                 cost_usd REAL, created_at INTEGER
             );
             INSERT INTO ai_os_providers
                 VALUES ('p', 'name', 'openai_compatible', '', NULL, 'm', 1, 0, 0);
             INSERT INTO ai_os_conversations
                 VALUES ('c', 't', 'p', 'm', 0, 0);
             INSERT INTO ai_os_messages
                 VALUES ('m', 'c', 'user', 'preexisting backfill marker', 0, 0, 0.0, 0);",
        )
        .unwrap();

        ensure_tables(&conn).unwrap();

        let hits = conversation_search(&conn, "backfill", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "c");
    }
}
