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
    line_end INTEGER NOT NULL,  -- Actually stores line_count (number of lines in chunk)
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

const CREATE_VEC_CHUNKS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
    chunk_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
)
"#;

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
    conn.execute(CREATE_VEC_CHUNKS, [])
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

    fn setup_connection() -> Connection {
        // Register sqlite-vec extension
        // SAFETY: See safety comment in sqlite.rs - same constraints apply
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_schema_constraints() {
        // Given a database with tables
        let conn = setup_connection();
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

    #[test]
    fn test_vec_chunks_virtual_table_created() {
        // Given A database with tables
        let conn = setup_connection();

        // When Creating tables (including vec_chunks virtual table)
        create_tables(&conn).unwrap();

        // Then The vec_chunks virtual table should exist
        let result: rusqlite::Result<String> = conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='vec_chunks'",
            [],
            |row| row.get(0),
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "vec_chunks");
    }
}
