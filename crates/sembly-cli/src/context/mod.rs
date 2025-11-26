//! Context management commands.
//!
//! This module provides CLI commands for managing contexts within a workspace.
//! Contexts enable multiple working directories to share a single embedding database.

use clap::{Parser, Subcommand};
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

use sembly_core::knowledge::KnowledgeIndex;

use super::theme;

/// Command for managing contexts within a workspace.
#[derive(Parser)]
pub struct ContextCommand {
    #[command(subcommand)]
    subcommand: ContextSubcommand,
}

/// Subcommands for context operations.
#[derive(Subcommand)]
enum ContextSubcommand {
    /// List all registered contexts.
    List(ListArgs),
    /// Remove a registered context.
    Remove(RemoveArgs),
}

/// Arguments for listing registered contexts.
#[derive(Parser)]
pub struct ListArgs {
    /// Path to forest root (defaults to current directory).
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Arguments for removing a registered context.
#[derive(Parser)]
pub struct RemoveArgs {
    /// The context identifier to remove.
    context_id: String,

    /// Path to forest root (defaults to current directory).
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Executes the context command by dispatching to the appropriate subcommand handler.
pub fn run(cmd: ContextCommand) -> Result<(), ContextError> {
    match cmd.subcommand {
        ContextSubcommand::List(args) => handle_list(args),
        ContextSubcommand::Remove(args) => handle_remove(args),
    }
}

/// Lists all registered contexts in the workspace.
fn handle_list(args: ListArgs) -> Result<(), ContextError> {
    use context_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let contexts = index.list_contexts().context(KnowledgeIndexSnafu)?;

    print!("{}", format_context_list(&contexts));

    Ok(())
}

/// Removes a registered context from the workspace.
fn handle_remove(args: RemoveArgs) -> Result<(), ContextError> {
    use context_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let context_id = sembly_core::knowledge::domain::ContextId::from_path(&args.context_id)
        .map_err(|_| ContextError::KnowledgeIndex {
            source: sembly_core::knowledge::facade::IndexError::ContextDoesNotExist {
                context_id: args.context_id.clone(),
            },
        })?;

    index
        .remove_context(&context_id)
        .context(KnowledgeIndexSnafu)?;

    println!(
        "{} {}",
        theme::label("Removed context:"),
        theme::value(args.context_id)
    );

    Ok(())
}

/// Formats the list of registered contexts as a displayable string.
///
/// The default context (`.`) is marked with "(default)".
fn format_context_list(contexts: &[sembly_core::knowledge::domain::Context]) -> String {
    if contexts.is_empty() {
        return String::new();
    }

    let mut output = format!("{}\n", theme::label("Registered contexts:"));
    for context in contexts {
        let id = context.context_id.as_str();
        if id == "." {
            output.push_str(&format!(
                "  {} {}\n",
                theme::value(id),
                theme::muted("(default)")
            ));
        } else {
            output.push_str(&format!("  {}\n", theme::value(id)));
        }
    }
    output
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum ContextError {
    #[snafu(display("Knowledge index operation failed"))]
    #[diagnostic(
        code(sembly::cli::context::knowledge_index_failed),
        help("Check the error details above for specific guidance")
    )]
    KnowledgeIndex {
        source: sembly_core::knowledge::facade::IndexError,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use sembly_core::knowledge::domain::{Context, ContextId};

    #[test]
    fn format_context_list_shows_default_context() {
        // Given a list containing only the default context
        let contexts = vec![
            Context::builder()
                .context_id(ContextId::from_path(".").unwrap())
                .build(),
        ];

        // When formatting the context list
        let output = format_context_list(&contexts);

        // Then the default context should be marked as "(default)"
        assert!(output.contains("."));
        assert!(output.contains("(default)"));
    }

    #[test]
    fn format_context_list_shows_multiple_contexts() {
        // Given a list with multiple contexts including the default
        let contexts = vec![
            Context::builder()
                .context_id(ContextId::from_path(".").unwrap())
                .build(),
            Context::builder()
                .context_id(ContextId::from_path("worktrees/feature-a").unwrap())
                .build(),
            Context::builder()
                .context_id(ContextId::from_path("worktrees/feature-b").unwrap())
                .build(),
        ];

        // When formatting the context list
        let output = format_context_list(&contexts);

        // Then all contexts should appear in the output
        assert!(output.contains("."));
        assert!(output.contains("(default)"));
        assert!(output.contains("worktrees/feature-a"));
        assert!(output.contains("worktrees/feature-b"));
        // And non-default contexts should not be marked as default
        let lines: Vec<&str> = output.lines().collect();
        let feature_a_line = lines.iter().find(|l| l.contains("feature-a")).unwrap();
        assert!(!feature_a_line.contains("(default)"));
    }

    #[test]
    fn format_context_list_empty_shows_nothing() {
        // Given an empty list of contexts
        let contexts: Vec<Context> = vec![];

        // When formatting the context list
        let output = format_context_list(&contexts);

        // Then the output should be empty
        assert!(output.is_empty());
    }
}
