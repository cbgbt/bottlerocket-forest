//! Database schema definitions and migrations
//!
//! Manages SQLite table creation and schema evolution for chunk storage.

use rusqlite::{Connection, OptionalExtension};
use snafu::{ResultExt, Snafu};

use crate::knowledge::constants::SCHEMA_VERSION;
use crate::knowledge::domain::EmbeddingModelConfig;

/// Errors that can occur during schema operations
#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum SchemaError {
    #[snafu(display("Failed to execute SQL"))]
    SqlExecution { source: rusqlite::Error },

    #[snafu(display(
        "Schema version mismatch: stored version {stored}, expected version {expected}"
    ))]
    #[snafu(visibility(pub))]
    #[diagnostic(
        code(sembly::schema::version_mismatch),
        help("Run `sembly rebuild` to recreate the index with the current schema")
    )]
    SchemaMismatch { stored: u32, expected: u32 },
}

type Result<T> = std::result::Result<T, SchemaError>;

const CREATE_INDEX_METADATA: &str = r#"
CREATE TABLE IF NOT EXISTS index_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
)
"#;

const CREATE_CONTEXTS: &str = r#"
CREATE TABLE IF NOT EXISTS contexts (
    context_id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    last_indexed INTEGER
)
"#;

const CREATE_INDEXED_FILES: &str = r#"
CREATE TABLE IF NOT EXISTS indexed_files (
    context_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash BLOB NOT NULL,
    mtime_ns INTEGER NOT NULL,
    PRIMARY KEY (context_id, file_path)
)
"#;

const CREATE_CHUNKS: &str = r#"
CREATE TABLE IF NOT EXISTS chunks (
    chunk_hash BLOB PRIMARY KEY,
    file_hash BLOB NOT NULL,
    repo_name TEXT NOT NULL,
    context_type TEXT NOT NULL CHECK(context_type IN ('markdown', 'rust_doc')),
    context_data TEXT NOT NULL,
    content TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    last_modified INTEGER NOT NULL
)
"#;

const CREATE_INDEX_FILE_HASH: &str =
    "CREATE INDEX IF NOT EXISTS idx_chunks_file_hash ON chunks(file_hash)";
const CREATE_INDEX_REPO: &str = "CREATE INDEX IF NOT EXISTS idx_chunks_repo ON chunks(repo_name)";

/// Reads the schema version from the index_metadata table
///
/// Returns `None` if no schema version has been set (legacy database).
pub fn get_schema_version(conn: &Connection) -> Result<Option<u32>> {
    use schema_error::*;

    let result = conn
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context(SqlExecutionSnafu)?;

    Ok(result.and_then(|s| s.parse().ok()))
}

/// Writes the schema version to the index_metadata table
pub fn set_schema_version(conn: &Connection, version: u32) -> Result<()> {
    use schema_error::*;

    conn.execute(
        "INSERT OR REPLACE INTO index_metadata (key, value) VALUES ('schema_version', ?)",
        [version.to_string()],
    )
    .context(SqlExecutionSnafu)?;
    Ok(())
}

/// Validates that the stored schema version matches the expected version
///
/// Returns an error if the versions do not match, indicating the index
/// needs to be rebuilt.
pub fn check_schema_version(conn: &Connection) -> Result<()> {
    use schema_error::*;

    let stored = get_schema_version(conn)?.unwrap_or(0);
    if stored != SCHEMA_VERSION {
        return SchemaMismatchSnafu {
            stored,
            expected: SCHEMA_VERSION,
        }
        .fail();
    }
    Ok(())
}

/// Initializes database schema including tables and indexes
pub fn create_tables(conn: &Connection, config: &EmbeddingModelConfig) -> Result<()> {
    use schema_error::*;

    conn.execute(CREATE_INDEX_METADATA, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_CONTEXTS, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEXED_FILES, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_CHUNKS, []).context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEX_FILE_HASH, [])
        .context(SqlExecutionSnafu)?;
    conn.execute(CREATE_INDEX_REPO, [])
        .context(SqlExecutionSnafu)?;

    let create_vec_chunks = format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
            chunk_hash TEXT PRIMARY KEY,
            embedding FLOAT[{}]
        )",
        config.embedding_dim
    );
    conn.execute(&create_vec_chunks, [])
        .context(SqlExecutionSnafu)?;

    set_schema_version(conn, SCHEMA_VERSION)?;

    Ok(())
}

/// Applies database migrations for schema evolution
pub fn migrate(_conn: &Connection) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::EmbeddingModelConfig;

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
    fn test_create_tables_with_default_config() {
        // Given A database connection and default config
        let conn = setup_connection();
        let config = EmbeddingModelConfig::default();

        // When Creating tables with the config
        let result = create_tables(&conn, &config);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_tables_with_custom_dimension() {
        // Given A database connection and custom embedding dimension
        let conn = setup_connection();
        let config = EmbeddingModelConfig::builder()
            .model_name("test-model")
            .embedding_dim(512)
            .max_tokens(256)
            .overlap_tokens(38)
            .build();

        // When Creating tables with the custom config
        let result = create_tables(&conn, &config);

        // Then It should succeed
        assert!(result.is_ok());

        // Then The vec_chunks table should be created with the correct dimension
        let table_info: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='vec_chunks'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(
            table_info.contains("FLOAT[512]"),
            "Expected vec_chunks table to use embedding_dim from config, got: {}",
            table_info
        );
    }

    #[test]
    fn contexts_table_has_correct_schema() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(contexts)").unwrap();
        let columns: Vec<(String, String, i32)> = stmt
            .query_map([], |row| Ok((row.get(1)?, row.get(2)?, row.get(5)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            columns[0],
            ("context_id".to_string(), "TEXT".to_string(), 1)
        ); // pk=1
        assert_eq!(
            columns[1],
            ("created_at".to_string(), "INTEGER".to_string(), 0)
        );
        assert_eq!(
            columns[2],
            ("last_indexed".to_string(), "INTEGER".to_string(), 0)
        );
    }

    #[test]
    fn indexed_files_table_has_correct_schema() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(indexed_files)").unwrap();
        let columns: Vec<(String, String, i32)> = stmt
            .query_map([], |row| Ok((row.get(1)?, row.get(2)?, row.get(5)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            columns[0],
            ("context_id".to_string(), "TEXT".to_string(), 1)
        ); // pk=1
        assert_eq!(columns[1], ("file_path".to_string(), "TEXT".to_string(), 2)); // pk=2
        assert_eq!(columns[2], ("file_hash".to_string(), "BLOB".to_string(), 0));
        assert_eq!(
            columns[3],
            ("mtime_ns".to_string(), "INTEGER".to_string(), 0)
        );
    }

    #[test]
    fn chunks_table_uses_chunk_hash_as_primary_key() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(chunks)").unwrap();
        let columns: Vec<(String, String, i32)> = stmt
            .query_map([], |row| Ok((row.get(1)?, row.get(2)?, row.get(5)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            columns[0],
            ("chunk_hash".to_string(), "BLOB".to_string(), 1)
        ); // pk=1
        assert_eq!(columns[1], ("file_hash".to_string(), "BLOB".to_string(), 0));
    }

    #[test]
    fn vec_chunks_table_uses_chunk_hash() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        let table_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='vec_chunks'",
                [],
                |_| Ok(true),
            )
            .unwrap();

        assert!(table_exists);
    }

    #[test]
    fn test_set_and_get_schema_version() {
        // Given a database with the index_metadata table
        let conn = setup_connection();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();

        // When setting a schema version
        set_schema_version(&conn, 42).unwrap();

        // Then get_schema_version should return that version
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, Some(42));
    }

    #[test]
    fn test_get_schema_version_returns_none_for_legacy_db() {
        // Given a database with index_metadata table but no schema_version row
        let conn = setup_connection();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();

        // When getting the schema version
        let version = get_schema_version(&conn).unwrap();

        // Then it should return None (legacy database)
        assert_eq!(version, None);
    }

    #[test]
    fn test_check_schema_version_passes_when_matching() {
        // Given a database with schema version set to SCHEMA_VERSION
        let conn = setup_connection();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
        set_schema_version(&conn, SCHEMA_VERSION).unwrap();

        // When checking the schema version
        let result = check_schema_version(&conn);

        // Then it should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_schema_version_fails_on_mismatch() {
        // Given a database with an old schema version (version 1)
        let conn = setup_connection();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
        set_schema_version(&conn, 1).unwrap();

        // When checking the schema version
        let result = check_schema_version(&conn);

        // Then it should fail with SchemaMismatch error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SchemaError::SchemaMismatch {
                stored: 1,
                expected: SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn test_create_tables_sets_schema_version() {
        // Given a fresh database connection
        let conn = setup_connection();
        let config = EmbeddingModelConfig::default();

        // When creating tables
        create_tables(&conn, &config).unwrap();

        // Then the schema version should be set to SCHEMA_VERSION
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, Some(SCHEMA_VERSION));
    }
}
