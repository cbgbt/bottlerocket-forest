//! Context resolution from current working directory.
//!
//! Determines which registered context the current directory belongs to
//! by finding the most specific matching context prefix.

use snafu::{ResultExt, Snafu, ensure};
use std::path::Path;

use crate::knowledge::domain::{Context, ContextId};
use crate::knowledge::storage::ContextRepository;

use super::Workspace;

/// Resolves the context for the current working directory.
///
/// Computes the relative path from workspace root to `cwd` and finds
/// the most specific registered context that is a prefix of this path.
pub fn resolve_context<R: ContextRepository>(
    workspace: &Workspace,
    cwd: &Path,
    repo: &R,
) -> Result<Context, ResolutionError> {
    use resolution_error::*;

    let rel_path = pathdiff::diff_paths(cwd, workspace.root()).ok_or_else(|| {
        PathOutsideWorkspaceSnafu {
            path: cwd.display().to_string(),
        }
        .build()
    })?;

    let cwd_context_id = ContextId::from_path(rel_path.to_str().unwrap_or(".")).map_err(|_| {
        PathOutsideWorkspaceSnafu {
            path: cwd.display().to_string(),
        }
        .build()
    })?;

    let contexts = repo.list_contexts().context(RepositorySnafu)?;

    let cwd_path = cwd_context_id.as_str();
    let mut matching: Vec<_> = contexts
        .into_iter()
        .filter(|ctx| {
            let ctx_path = ctx.context_id.as_str();
            cwd_path == ctx_path
                || (cwd_path.starts_with(ctx_path)
                    && cwd_path.as_bytes().get(ctx_path.len()) == Some(&b'/'))
        })
        .collect();

    ensure!(
        !matching.is_empty(),
        NoMatchingContextSnafu {
            available_contexts: repo
                .list_contexts()
                .context(RepositorySnafu)?
                .into_iter()
                .map(|c| c.context_id)
                .collect::<Vec<_>>(),
        }
    );

    // Sort by path length descending to get the most specific (longest) matching context first
    matching.sort_by_key(|ctx| std::cmp::Reverse(ctx.context_id.as_str().len()));
    Ok(matching.into_iter().next().unwrap())
}

/// Errors that can occur during context resolution.
#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ResolutionError {
    /// The current directory is outside the workspace.
    #[snafu(display("Path is outside workspace: {path}"))]
    PathOutsideWorkspace { path: String },

    /// No registered context matches the current directory.
    #[snafu(display("No matching context for current directory"))]
    NoMatchingContext { available_contexts: Vec<ContextId> },

    /// Failed to query the context repository.
    #[snafu(display("Failed to query contexts"))]
    RepositoryError {
        source: crate::knowledge::storage::ContextRepositoryError,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::storage::repository::MockContextRepository;
    use std::fs;
    use tempfile::TempDir;

    fn create_workspace(root: &std::path::Path) -> Workspace {
        let sembly_dir = root.join(".sembly");
        fs::create_dir_all(&sembly_dir).unwrap();
        fs::File::create(sembly_dir.join("knowledge.db")).unwrap();
        Workspace::new(root.to_path_buf())
    }

    #[test]
    fn resolves_to_exact_context_match() {
        // Given a workspace with a registered context "subdir"
        let temp = TempDir::new().unwrap();
        let workspace = create_workspace(temp.path());
        let subdir = temp.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let mut mock_repo = MockContextRepository::new();
        mock_repo.expect_list_contexts().returning(|| {
            Ok(vec![
                Context::builder()
                    .context_id(ContextId::from_path("subdir").unwrap())
                    .build(),
            ])
        });

        // When resolving context from the exact subdir path
        let result = resolve_context(&workspace, &subdir, &mock_repo);

        // Then it should return the matching context
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), "subdir");
    }

    #[test]
    fn resolves_to_parent_context_when_in_subdirectory() {
        // Given a workspace with context "projects" and cwd is "projects/foo/bar"
        let temp = TempDir::new().unwrap();
        let workspace = create_workspace(temp.path());
        let nested = temp.path().join("projects").join("foo").join("bar");
        fs::create_dir_all(&nested).unwrap();

        let mut mock_repo = MockContextRepository::new();
        mock_repo.expect_list_contexts().returning(|| {
            Ok(vec![
                Context::builder()
                    .context_id(ContextId::from_path("projects").unwrap())
                    .build(),
            ])
        });

        // When resolving context from the nested subdirectory
        let result = resolve_context(&workspace, &nested, &mock_repo);

        // Then it should return the parent context "projects"
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), "projects");
    }

    #[test]
    fn returns_error_listing_contexts_when_no_match() {
        // Given a workspace with contexts "alpha" and "beta", but cwd is "gamma"
        let temp = TempDir::new().unwrap();
        let workspace = create_workspace(temp.path());
        let gamma = temp.path().join("gamma");
        fs::create_dir_all(&gamma).unwrap();

        let mut mock_repo = MockContextRepository::new();
        mock_repo.expect_list_contexts().returning(|| {
            Ok(vec![
                Context::builder()
                    .context_id(ContextId::from_path("alpha").unwrap())
                    .build(),
                Context::builder()
                    .context_id(ContextId::from_path("beta").unwrap())
                    .build(),
            ])
        });

        // When resolving context from an unregistered directory
        let result = resolve_context(&workspace, &gamma, &mock_repo);

        // Then it should return NoMatchingContext with available contexts
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ResolutionError::NoMatchingContext { available_contexts }
            if available_contexts.len() == 2
        ));
    }

    #[test]
    fn handles_workspace_root_context() {
        // Given a workspace with the default "." context registered
        let temp = TempDir::new().unwrap();
        let workspace = create_workspace(temp.path());

        let mut mock_repo = MockContextRepository::new();
        mock_repo.expect_list_contexts().returning(|| {
            Ok(vec![
                Context::builder()
                    .context_id(ContextId::from_path(".").unwrap())
                    .build(),
            ])
        });

        // When resolving context from the workspace root
        let result = resolve_context(&workspace, temp.path(), &mock_repo);

        // Then it should return the "." context
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), ".");
    }

    #[test]
    fn returns_most_specific_matching_context() {
        // Given a workspace with contexts "." and "projects" and "projects/core"
        let temp = TempDir::new().unwrap();
        let workspace = create_workspace(temp.path());
        let deep = temp.path().join("projects").join("core").join("src");
        fs::create_dir_all(&deep).unwrap();

        let mut mock_repo = MockContextRepository::new();
        mock_repo.expect_list_contexts().returning(|| {
            Ok(vec![
                Context::builder()
                    .context_id(ContextId::from_path(".").unwrap())
                    .build(),
                Context::builder()
                    .context_id(ContextId::from_path("projects").unwrap())
                    .build(),
                Context::builder()
                    .context_id(ContextId::from_path("projects/core").unwrap())
                    .build(),
            ])
        });

        // When resolving context from projects/core/src
        let result = resolve_context(&workspace, &deep, &mock_repo);

        // Then it should return the most specific context "projects/core"
        let context = result.unwrap();
        assert_eq!(context.context_id.as_str(), "projects/core");
    }
}
