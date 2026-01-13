//! Core domain types for forester.
//!
//! This module contains type-safe domain models with no I/O dependencies.
//! All types use validation via nutype and builders via bon.

mod config;
mod forest;
mod grove;
mod member;

pub use config::{ForestConfig, ForestMeta, GroveConfig, HookConfig, Member, SymlinkEntry};
pub use forest::{ForestName, ForestRoot};
pub use grove::{GroveName, GroveRoot};
pub use member::{BranchName, MemberName, Remote};
