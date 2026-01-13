//! Forester provides generic forest management for multi-repo projects.
//!
//! A forest is a collection of related git repositories that are developed together
//! but maintained as separate repos (not submodules). Forester provides:
//!
//! - Forest configuration via `forester.toml`
//! - Coordinated worktree management across all member repos
//! - Integration with crumbly for semantic search

pub mod cli;
pub mod domain;
pub mod events;
pub mod forest;
pub mod git;
pub mod grove;
pub mod hooks;

pub use domain::config::ForestConfig;
