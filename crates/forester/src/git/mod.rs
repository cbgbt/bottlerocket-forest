//! Git abstraction layer for forester.
//!
//! Provides operations for bare repositories and worktrees.

mod bare;
mod worktree;

pub use bare::{BareRepository, CloneBareError};
pub use worktree::{CreateWorktreeError, RemoveWorktreeError};
