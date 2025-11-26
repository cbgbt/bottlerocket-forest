//! Context management for multi-context indexing.
//!
//! This module provides workspace discovery and context resolution,
//! enabling multiple working directories to share a single embedding database.

mod discovery;
mod resolver;

pub use discovery::{DiscoveryError, Workspace, discover_workspace};
pub use resolver::{ResolutionError, resolve_context};
