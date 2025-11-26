//! SQLite implementation of ContextRepository
//!
//! Provides persistent storage for context management in multi-context indexing.

use rusqlite::Connection;
use snafu::ResultExt;

use crate::knowledge::domain::{Context, ContextId};
use crate::knowledge::storage::repository::{ContextRepository, ContextRepositoryError};

/// SQLite-backed implementation of context repository
pub struct SqliteContextRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> SqliteContextRepository<'conn> {
    /// Creates a new repository using the provided connection
    pub fn new(conn: &'conn Connection) -> Self {
        Self { conn }
    }
}

impl ContextRepository for SqliteContextRepository<'_> {
    fn list_contexts(&self) -> Result<Vec<Context>, ContextRepositoryError> {
        use crate::knowledge::storage::repository::context_repository_error::*;

        let mut stmt = self
            .conn
            .prepare("SELECT context_id, created_at, last_indexed FROM contexts")
            .context(DatabaseSnafu)?;

        let contexts = stmt
            .query_map([], |row| {
                let context_id: String = row.get(0)?;
                let created_at_ts: i64 = row.get(1)?;
                let last_indexed_ts: Option<i64> = row.get(2)?;

                Ok(Context {
                    context_id: ContextId::from_path(&context_id)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    created_at: chrono::DateTime::from_timestamp(created_at_ts, 0)
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                    last_indexed: last_indexed_ts
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
                })
            })
            .context(DatabaseSnafu)?
            .collect::<Result<Vec<_>, _>>()
            .context(DatabaseSnafu)?;

        Ok(contexts)
    }

    fn get_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Option<Context>, ContextRepositoryError> {
        use crate::knowledge::storage::repository::context_repository_error::*;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT context_id, created_at, last_indexed FROM contexts WHERE context_id = ?",
            )
            .context(DatabaseSnafu)?;

        let result = stmt.query_row([context_id.as_str()], |row| {
            let context_id: String = row.get(0)?;
            let created_at_ts: i64 = row.get(1)?;
            let last_indexed_ts: Option<i64> = row.get(2)?;

            Ok(Context {
                context_id: ContextId::from_path(&context_id)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                created_at: chrono::DateTime::from_timestamp(created_at_ts, 0)
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                last_indexed: last_indexed_ts
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            })
        });

        match result {
            Ok(context) => Ok(Some(context)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context(DatabaseSnafu),
        }
    }

    fn insert_context(&self, context: &Context) -> Result<(), ContextRepositoryError> {
        use crate::knowledge::storage::repository::context_repository_error::*;

        let created_at_ts = context.created_at.timestamp();
        let last_indexed_ts = context.last_indexed.map(|dt| dt.timestamp());

        let result = self.conn.execute(
            "INSERT INTO contexts (context_id, created_at, last_indexed) VALUES (?, ?, ?)",
            rusqlite::params![context.context_id.as_str(), created_at_ts, last_indexed_ts],
        );

        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation
                    && err.extended_code == 1555 =>
            {
                Err(ContextRepositoryError::AlreadyExists {
                    context_id: context.context_id.as_str().to_string(),
                })
            }
            Err(e) => Err(e).context(DatabaseSnafu),
        }
    }

    fn remove_context(&self, context_id: &ContextId) -> Result<(), ContextRepositoryError> {
        use crate::knowledge::storage::repository::context_repository_error::*;

        self.conn
            .execute(
                "DELETE FROM indexed_files WHERE context_id = ?",
                [context_id.as_str()],
            )
            .context(DatabaseSnafu)?;

        self.conn
            .execute(
                "DELETE FROM contexts WHERE context_id = ?",
                [context_id.as_str()],
            )
            .context(DatabaseSnafu)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;
    use crate::knowledge::domain::EmbeddingModelConfig;
    use crate::knowledge::storage::schema::create_tables;

    fn setup_connection() -> rusqlite::Connection {
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_tables(&conn, &EmbeddingModelConfig::default()).unwrap();
        conn
    }

    #[test]
    fn insert_and_retrieve_context() {
        // Given a repository and a context
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);
        let context = Context::builder()
            .context_id(ContextId::from_path("my-project").unwrap())
            .build();

        // When inserting and then retrieving the context
        repo.insert_context(&context).unwrap();
        let retrieved = repo.get_context(&context.context_id).unwrap();

        // Then the retrieved context should match the inserted one
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.context_id, context.context_id);
    }

    #[test]
    fn get_context_returns_none_for_missing() {
        // Given a repository with no contexts
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);
        let context_id = ContextId::from_path("nonexistent").unwrap();

        // When retrieving a non-existent context
        let result = repo.get_context(&context_id).unwrap();

        // Then it should return None
        assert!(result.is_none());
    }

    #[test]
    fn list_multiple_contexts() {
        // Given a repository with multiple contexts
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);

        for id in ["project-a", "project-b", "worktrees/feature-x"] {
            let ctx = Context::builder()
                .context_id(ContextId::from_path(id).unwrap())
                .build();
            repo.insert_context(&ctx).unwrap();
        }

        // When listing all contexts
        let contexts = repo.list_contexts().unwrap();

        // Then all inserted contexts should be returned
        assert_eq!(contexts.len(), 3);
        let ids: HashSet<_> = contexts.iter().map(|c| c.context_id.as_str()).collect();
        assert!(ids.contains("project-a"));
        assert!(ids.contains("project-b"));
        assert!(ids.contains("worktrees/feature-x"));
    }

    #[test]
    fn list_contexts_empty() {
        // Given a repository with no contexts
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);

        // When listing contexts
        let contexts = repo.list_contexts().unwrap();

        // Then it should return an empty list
        assert!(contexts.is_empty());
    }

    #[test]
    fn remove_context() {
        // Given a repository with an inserted context
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);
        let context = Context::builder()
            .context_id(ContextId::from_path("to-remove").unwrap())
            .build();
        repo.insert_context(&context).unwrap();

        // When removing the context
        repo.remove_context(&context.context_id).unwrap();

        // Then get_context should return None
        let result = repo.get_context(&context.context_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn duplicate_insert_fails_with_already_exists() {
        // Given a repository with an existing context
        let conn = setup_connection();
        let repo = SqliteContextRepository::new(&conn);
        let context = Context::builder()
            .context_id(ContextId::from_path("duplicate").unwrap())
            .build();
        repo.insert_context(&context).unwrap();

        // When inserting the same context again
        let result = repo.insert_context(&context);

        // Then it should fail with AlreadyExists error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ContextRepositoryError::AlreadyExists { .. }));
    }
}
