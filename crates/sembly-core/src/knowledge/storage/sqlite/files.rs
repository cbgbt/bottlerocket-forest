//! Storage queries for indexed files.
//!
//! Provides CRUD operations for IndexedFile records, which track
//! file presence within contexts for content-addressed storage.

use rusqlite::Connection;
use snafu::{ResultExt, Snafu};

use crate::knowledge::domain::{ContextId, FileHash, ForestRelativePath, IndexedFile};

/// Errors that can occur during indexed file storage operations.
#[derive(Debug, Snafu)]
#[snafu(module, visibility(pub))]
pub enum IndexedFileError {
    /// Database operation failed.
    #[snafu(display("Database operation failed"))]
    Database { source: rusqlite::Error },

    /// Invalid data retrieved from database.
    #[snafu(display("Invalid data in database: {message}"))]
    InvalidData { message: String },
}

/// Inserts or updates an indexed file record.
pub fn insert_indexed_file(conn: &Connection, file: &IndexedFile) -> Result<(), IndexedFileError> {
    use indexed_file_error::*;

    conn.execute(
        "INSERT OR REPLACE INTO indexed_files (context_id, file_path, file_hash, mtime_ns) VALUES (?, ?, ?, ?)",
        rusqlite::params![
            file.context_id.as_str(),
            file.file_path.to_string(),
            file.file_hash.as_bytes(),
            file.mtime_ns
        ],
    )
    .context(DatabaseSnafu)?;

    Ok(())
}

/// Retrieves an indexed file by context and path.
pub fn get_indexed_file(
    conn: &Connection,
    context_id: &ContextId,
    file_path: &ForestRelativePath,
) -> Result<Option<IndexedFile>, IndexedFileError> {
    use indexed_file_error::*;

    let mut stmt = conn
        .prepare("SELECT context_id, file_path, file_hash, mtime_ns FROM indexed_files WHERE context_id = ? AND file_path = ?")
        .context(DatabaseSnafu)?;

    let result = stmt.query_row(
        rusqlite::params![context_id.as_str(), file_path.to_string()],
        |row| {
            let context_id: String = row.get(0)?;
            let file_path: String = row.get(1)?;
            let file_hash_bytes: Vec<u8> = row.get(2)?;
            let mtime_ns: i64 = row.get(3)?;

            let file_hash_array: [u8; 32] = file_hash_bytes
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            Ok(IndexedFile {
                context_id: ContextId::from_path(&context_id)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                file_path: ForestRelativePath::try_new(&file_path)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                file_hash: FileHash::new(file_hash_array),
                mtime_ns,
            })
        },
    );

    match result {
        Ok(file) => Ok(Some(file)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e).context(DatabaseSnafu),
    }
}

/// Lists all indexed files for a context.
pub fn list_indexed_files_for_context(
    conn: &Connection,
    context_id: &ContextId,
) -> Result<Vec<IndexedFile>, IndexedFileError> {
    use indexed_file_error::*;

    let mut stmt = conn
        .prepare("SELECT context_id, file_path, file_hash, mtime_ns FROM indexed_files WHERE context_id = ?")
        .context(DatabaseSnafu)?;

    let files = stmt
        .query_map([context_id.as_str()], |row| {
            let context_id: String = row.get(0)?;
            let file_path: String = row.get(1)?;
            let file_hash_bytes: Vec<u8> = row.get(2)?;
            let mtime_ns: i64 = row.get(3)?;

            let file_hash_array: [u8; 32] = file_hash_bytes
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            Ok(IndexedFile {
                context_id: ContextId::from_path(&context_id)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                file_path: ForestRelativePath::try_new(&file_path)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                file_hash: FileHash::new(file_hash_array),
                mtime_ns,
            })
        })
        .context(DatabaseSnafu)?
        .collect::<Result<Vec<_>, _>>()
        .context(DatabaseSnafu)?;

    Ok(files)
}

/// Deletes all indexed files for a context.
pub fn delete_indexed_files_for_context(
    conn: &Connection,
    context_id: &ContextId,
) -> Result<usize, IndexedFileError> {
    use indexed_file_error::*;

    let deleted = conn
        .execute(
            "DELETE FROM indexed_files WHERE context_id = ?",
            [context_id.as_str()],
        )
        .context(DatabaseSnafu)?;

    Ok(deleted)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{EmbeddingModelConfig, FileHash};
    use crate::knowledge::storage::schema;
    use rusqlite::Connection;
    use std::io::Cursor;

    fn setup_connection() -> Connection {
        // SAFETY: This call satisfies the safety requirements for sqlite3_auto_extension:
        // 1. We are not calling this from within an auto-extension handler
        // 2. We will not close any database connection from within the auto-extension
        // 3. We will not manipulate the auto-extension list from within an auto-extension
        // 4. sqlite3_vec_init is a valid C function pointer provided by the sqlite-vec crate
        // 5. The transmute is valid because both types are function pointers with the same size
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let conn = Connection::open_in_memory().unwrap();
        schema::create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();
        conn
    }

    fn make_file_hash(content: &[u8]) -> FileHash {
        FileHash::from_reader(Cursor::new(content)).unwrap()
    }

    #[test]
    fn indexed_file_creation_with_all_fields() {
        // Given valid components for an IndexedFile
        let context_id = ContextId::from_path("worktrees/feature-a").unwrap();
        let file_path = ForestRelativePath::try_new("src/main.rs").unwrap();
        let file_hash = make_file_hash(b"fn main() {}");
        let mtime_ns = 1700000000_000_000_000i64;

        // When creating an IndexedFile with all fields
        let indexed_file = IndexedFile::builder()
            .context_id(context_id.clone())
            .file_path(file_path.clone())
            .file_hash(file_hash)
            .mtime_ns(mtime_ns)
            .build();

        // Then all fields should be accessible
        assert_eq!(indexed_file.context_id, context_id);
        assert_eq!(indexed_file.file_path, file_path);
        assert_eq!(indexed_file.file_hash, file_hash);
        assert_eq!(indexed_file.mtime_ns, mtime_ns);
    }

    #[test]
    fn insert_and_retrieve_indexed_file() {
        // Given a database and an indexed file
        let conn = setup_connection();
        let context_id = ContextId::from_path(".").unwrap();
        let file_path = ForestRelativePath::try_new("docs/README.md").unwrap();
        let file_hash = make_file_hash(b"# README");
        let mtime_ns = 1700000000_000_000_000i64;

        let indexed_file = IndexedFile::builder()
            .context_id(context_id.clone())
            .file_path(file_path.clone())
            .file_hash(file_hash)
            .mtime_ns(mtime_ns)
            .build();

        // When inserting the file
        insert_indexed_file(&conn, &indexed_file).unwrap();

        // Then it should be retrievable
        let retrieved = get_indexed_file(&conn, &context_id, &file_path)
            .unwrap()
            .expect("file should exist");

        assert_eq!(retrieved.context_id, context_id);
        assert_eq!(retrieved.file_path, file_path);
        assert_eq!(retrieved.file_hash, file_hash);
        assert_eq!(retrieved.mtime_ns, mtime_ns);
    }

    #[test]
    fn lists_files_for_specific_context() {
        // Given a database with files in multiple contexts
        let conn = setup_connection();
        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        let file1 = IndexedFile::builder()
            .context_id(context_a.clone())
            .file_path(ForestRelativePath::try_new("file1.rs").unwrap())
            .file_hash(make_file_hash(b"content1"))
            .mtime_ns(1000)
            .build();

        let file2 = IndexedFile::builder()
            .context_id(context_a.clone())
            .file_path(ForestRelativePath::try_new("file2.rs").unwrap())
            .file_hash(make_file_hash(b"content2"))
            .mtime_ns(2000)
            .build();

        let file3 = IndexedFile::builder()
            .context_id(context_b.clone())
            .file_path(ForestRelativePath::try_new("file3.rs").unwrap())
            .file_hash(make_file_hash(b"content3"))
            .mtime_ns(3000)
            .build();

        insert_indexed_file(&conn, &file1).unwrap();
        insert_indexed_file(&conn, &file2).unwrap();
        insert_indexed_file(&conn, &file3).unwrap();

        // When listing files for context_a
        let files = list_indexed_files_for_context(&conn, &context_a).unwrap();

        // Then only files from context_a should be returned
        assert_eq!(files.len(), 2);
        let paths: Vec<_> = files.iter().map(|f| f.file_path.to_string()).collect();
        assert!(paths.contains(&"file1.rs".to_string()));
        assert!(paths.contains(&"file2.rs".to_string()));
    }

    #[test]
    fn deletes_files_for_specific_context() {
        // Given a database with files in multiple contexts
        let conn = setup_connection();
        let context_a = ContextId::from_path("context-a").unwrap();
        let context_b = ContextId::from_path("context-b").unwrap();

        let file1 = IndexedFile::builder()
            .context_id(context_a.clone())
            .file_path(ForestRelativePath::try_new("file1.rs").unwrap())
            .file_hash(make_file_hash(b"content1"))
            .mtime_ns(1000)
            .build();

        let file2 = IndexedFile::builder()
            .context_id(context_b.clone())
            .file_path(ForestRelativePath::try_new("file2.rs").unwrap())
            .file_hash(make_file_hash(b"content2"))
            .mtime_ns(2000)
            .build();

        insert_indexed_file(&conn, &file1).unwrap();
        insert_indexed_file(&conn, &file2).unwrap();

        // When deleting files for context_a
        let deleted_count = delete_indexed_files_for_context(&conn, &context_a).unwrap();

        // Then only context_a files should be deleted
        assert_eq!(deleted_count, 1);

        // And context_b files should remain
        let remaining = list_indexed_files_for_context(&conn, &context_b).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].file_path.to_string(), "file2.rs");
    }

    #[test]
    fn same_file_path_in_different_contexts_creates_separate_records() {
        // Given a database
        let conn = setup_connection();
        let context_a = ContextId::from_path("worktree-a").unwrap();
        let context_b = ContextId::from_path("worktree-b").unwrap();
        let same_path = ForestRelativePath::try_new("src/lib.rs").unwrap();

        // When inserting the same file path in different contexts
        let file_a = IndexedFile::builder()
            .context_id(context_a.clone())
            .file_path(same_path.clone())
            .file_hash(make_file_hash(b"version A content"))
            .mtime_ns(1000)
            .build();

        let file_b = IndexedFile::builder()
            .context_id(context_b.clone())
            .file_path(same_path.clone())
            .file_hash(make_file_hash(b"version B content"))
            .mtime_ns(2000)
            .build();

        insert_indexed_file(&conn, &file_a).unwrap();
        insert_indexed_file(&conn, &file_b).unwrap();

        // Then both records should exist independently
        let retrieved_a = get_indexed_file(&conn, &context_a, &same_path)
            .unwrap()
            .expect("file in context_a should exist");
        let retrieved_b = get_indexed_file(&conn, &context_b, &same_path)
            .unwrap()
            .expect("file in context_b should exist");

        // And they should have different file hashes
        assert_ne!(retrieved_a.file_hash, retrieved_b.file_hash);
        assert_eq!(retrieved_a.mtime_ns, 1000);
        assert_eq!(retrieved_b.mtime_ns, 2000);
    }
}
