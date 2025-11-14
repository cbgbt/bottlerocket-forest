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
    line_count INTEGER NOT NULL,
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
        // SAFETY: This call satisfies the safety requirements for sqlite3_auto_extension:
        // 1. We are not calling this from within an auto-extension handler
        // 2. We will not close any database connection from within the auto-extension
        // 3. We will not manipulate the auto-extension list from within an auto-extension
        // 4. sqlite3_vec_init is a valid C function pointer provided by the sqlite-vec crate
        // 5. The transmute is valid because both types are function pointers with the same size
        //    and sqlite3_vec_init has the correct signature expected by sqlite3_auto_extension
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_create_tables_succeeds() {
        // Given A database connection
        let conn = setup_connection();

        // When Creating tables
        let result = create_tables(&conn);

        // Then It should succeed
        assert!(result.is_ok());
    }
}
