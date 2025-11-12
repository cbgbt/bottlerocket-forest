//! Database schema definitions and migrations

use rusqlite::Connection;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum SchemaError {
    #[snafu(display("Failed to execute SQL"))]
    SqlExecution { source: rusqlite::Error },
}

type Result<T> = std::result::Result<T, SchemaError>;

const CREATE_INDEX_METADATA: &str = r#"
CREATE TABLE IF NOT EXISTS index_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
)
"#;

const CREATE_CHUNKS: &str = r#"
CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    repo_name TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    context_type TEXT NOT NULL CHECK(context_type IN ('markdown', 'rust_doc')),
    context_data TEXT NOT NULL,
    content TEXT NOT NULL,
    last_modified INTEGER NOT NULL,
    embedding BLOB,
    bm25_terms TEXT,
    index_mode TEXT NOT NULL CHECK(index_mode IN ('fast', 'best'))
)
"#;

const CREATE_INDEX_FILE: &str = "CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path)";
const CREATE_INDEX_REPO: &str = "CREATE INDEX IF NOT EXISTS idx_chunks_repo ON chunks(repo_name)";
const CREATE_INDEX_MODE: &str = "CREATE INDEX IF NOT EXISTS idx_chunks_mode ON chunks(index_mode)";

/// Creates all tables and indexes in the database
pub fn create_tables(conn: &Connection) -> Result<()> {
    use schema_error::*;

    conn.execute(CREATE_INDEX_METADATA, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_CHUNKS, []).context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEX_FILE, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEX_REPO, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEX_MODE, [])
        .context(SqlExecutionSnafu)?;

    Ok(())
}

/// Runs database migrations
pub fn migrate(_conn: &Connection) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_schema_constraints() {
        // Given a database with tables
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();

        // When inserting a chunk with invalid context_type
        let result = conn.execute(
            "INSERT INTO chunks (id, file_path, repo_name, line_start, line_end, 
             context_type, context_data, content, last_modified, index_mode) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                "test-id",
                "test.md",
                "test-repo",
                1,
                10,
                "invalid_type",
                "{}",
                "test content",
                0,
                "fast"
            ],
        );

        // Then it should fail due to CHECK constraint
        assert!(result.is_err());
    }
}
