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
#[non_exhaustive]
pub enum SchemaError {
    /// SQL statement execution failed.
    #[snafu(display("Failed to execute SQL"))]
    SqlExecution {
        /// Underlying database error.
        source: rusqlite::Error,
    },

    /// Database schema version incompatible with application.
    #[snafu(display(
        "Schema version mismatch: stored version {stored}, expected version {expected}"
    ))]
    #[snafu(visibility(pub))]
    #[diagnostic(
        code(crumbly::schema::version_mismatch),
        help("Run `crumbly rebuild` to recreate the index with the current schema")
    )]
    SchemaMismatch {
        /// Version found in the database.
        stored: u32,
        /// Version expected by the application.
        expected: u32,
    },
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
    context_type TEXT NOT NULL,
    context_data TEXT NOT NULL,
    content TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    last_modified INTEGER NOT NULL
)
"#;

const CREATE_INDEX_FILE_HASH: &str =
    "CREATE INDEX IF NOT EXISTS idx_chunks_file_hash ON chunks(file_hash)";
const CREATE_INDEX_REPO: &str = "CREATE INDEX IF NOT EXISTS idx_chunks_repo ON chunks(repo_name)";
const CREATE_INDEX_CONTEXT_TYPE: &str =
    "CREATE INDEX IF NOT EXISTS idx_chunks_context_type ON chunks(context_type)";

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
/// Attempts migration if possible, otherwise returns an error indicating
/// the index needs to be rebuilt.
pub fn check_schema_version(conn: &Connection) -> Result<()> {
    use schema_error::*;

    let stored = get_schema_version(conn)?.unwrap_or(0);
    if stored == SCHEMA_VERSION {
        return Ok(());
    }

    SchemaMismatchSnafu {
        stored,
        expected: SCHEMA_VERSION,
    }
    .fail()
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
    conn.execute(CREATE_INDEX_CONTEXT_TYPE, [])
        .context(SqlExecutionSnafu)?;

    let create_vec_chunks = format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
            chunk_hash TEXT PRIMARY KEY,
            embedding FLOAT[{}] distance_metric=cosine
        )",
        config.embedding_dim
    );
    conn.execute(&create_vec_chunks, [])
        .context(SqlExecutionSnafu)?;

    set_schema_version(conn, SCHEMA_VERSION)?;

    Ok(())
}

/// Migrates from schema v3 to v4
///
/// v4 removes the CHECK constraint from context_type, allowing any string value.
#[cfg(test)]
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    use schema_error::*;

    conn.execute("BEGIN TRANSACTION", [])
        .context(SqlExecutionSnafu)?;

    let result = (|| -> Result<()> {
        conn.execute(
            r#"CREATE TABLE chunks_new (
    chunk_hash BLOB PRIMARY KEY,
    file_hash BLOB NOT NULL,
    repo_name TEXT NOT NULL,
    context_type TEXT NOT NULL,
    context_data TEXT NOT NULL,
    content TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    last_modified INTEGER NOT NULL
)"#,
            [],
        )
        .context(SqlExecutionSnafu)?;
        conn.execute("INSERT INTO chunks_new SELECT * FROM chunks", [])
            .context(SqlExecutionSnafu)?;
        conn.execute("DROP TABLE chunks", [])
            .context(SqlExecutionSnafu)?;
        conn.execute("ALTER TABLE chunks_new RENAME TO chunks", [])
            .context(SqlExecutionSnafu)?;
        conn.execute(CREATE_INDEX_FILE_HASH, [])
            .context(SqlExecutionSnafu)?;
        conn.execute(CREATE_INDEX_REPO, [])
            .context(SqlExecutionSnafu)?;
        conn.execute(CREATE_INDEX_CONTEXT_TYPE, [])
            .context(SqlExecutionSnafu)?;
        set_schema_version(conn, 4)?;
        Ok(())
    })();

    match result {
        Ok(_) => {
            conn.execute("COMMIT", []).context(SqlExecutionSnafu)?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
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
    fn tables_have_correct_schema() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        // Verify contexts table
        let ctx_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(contexts)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(ctx_cols, vec!["context_id", "created_at", "last_indexed"]);

        // Verify chunks table primary key
        let chunk_pk: i32 = conn
            .prepare("PRAGMA table_info(chunks)")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
            })
            .unwrap()
            .find(|r| r.as_ref().map(|(n, _)| n == "chunk_hash").unwrap_or(false))
            .unwrap()
            .unwrap()
            .1;
        assert_eq!(chunk_pk, 1);

        // Verify vec_chunks exists
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='vec_chunks'",
                [],
                |_| Ok(true),
            )
            .unwrap();
        assert!(exists);
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

    fn setup_metadata_table(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn test_schema_version_operations() {
        // Given a database with index_metadata table
        let conn = setup_connection();
        setup_metadata_table(&conn);

        // Then get_schema_version returns None for legacy db
        assert_eq!(get_schema_version(&conn).unwrap(), None);

        // When setting version, check_schema_version passes for matching version
        set_schema_version(&conn, SCHEMA_VERSION).unwrap();
        assert!(check_schema_version(&conn).is_ok());

        // When version mismatches, check_schema_version fails
        set_schema_version(&conn, 1).unwrap();
        let err = check_schema_version(&conn).unwrap_err();
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

    fn create_v3_schema(conn: &Connection) {
        conn.execute(CREATE_INDEX_METADATA, []).unwrap();
        conn.execute(
            r#"CREATE TABLE chunks (
    chunk_hash BLOB PRIMARY KEY, file_hash BLOB NOT NULL, repo_name TEXT NOT NULL,
    context_type TEXT NOT NULL CHECK(context_type IN ('markdown', 'rust_doc', 'go_doc')),
    context_data TEXT NOT NULL, content TEXT NOT NULL, token_count INTEGER NOT NULL,
    last_modified INTEGER NOT NULL)"#,
            [],
        )
        .unwrap();
        conn.execute(CREATE_INDEX_FILE_HASH, []).unwrap();
        conn.execute(CREATE_INDEX_REPO, []).unwrap();
        set_schema_version(&conn, 3).unwrap();
    }

    #[test]
    fn test_migrate_v3_to_v4_succeeds() {
        // Given a v3 schema database
        let conn = setup_connection();
        create_v3_schema(&conn);

        // When migrating to v4
        migrate_v3_to_v4(&conn).unwrap();

        // Then schema version should be 4
        assert_eq!(get_schema_version(&conn).unwrap(), Some(4));
    }

    #[test]
    fn test_migrate_v3_to_v4_preserves_data() {
        // Given a v3 database with existing chunk
        let conn = setup_connection();
        create_v3_schema(&conn);
        conn.execute(
            "INSERT INTO chunks VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &[1u8][..],
                &[2u8][..],
                "repo",
                "markdown",
                "{}",
                "content",
                10,
                123
            ],
        )
        .unwrap();

        // When migrating to v4
        migrate_v3_to_v4(&conn).unwrap();

        // Then data should be preserved
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrate_v3_to_v4_allows_new_context_types() {
        // Given a v4 database (migrated from v3)
        let conn = setup_connection();
        create_v3_schema(&conn);
        migrate_v3_to_v4(&conn).unwrap();

        // When inserting a new context type
        let result = conn.execute(
            "INSERT INTO chunks VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &[1u8][..],
                &[2u8][..],
                "repo",
                "java_doc",
                "{}",
                "content",
                10,
                123
            ],
        );

        // Then it should succeed (no CHECK constraint)
        assert!(result.is_ok());
    }

    #[test]
    fn test_vec_chunks_uses_cosine_distance() {
        let conn = setup_connection();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();

        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='vec_chunks'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(
            sql.contains("distance_metric=cosine"),
            "Expected vec_chunks DDL to contain distance_metric=cosine, got: {}",
            sql
        );
    }
}
