//! Operations layer for forester.
//!
//! Each operation takes references to EventEmitter and HookRegistry,
//! uses domain types, and emits events for user feedback.

mod seed;
mod grove_create;
mod grove_remove;
mod grove_list;
mod init;

pub use seed::{SeedOperation, SeedError};
pub use grove_create::{GroveCreateOperation, GroveCreateError};
pub use grove_remove::{GroveRemoveOperation, GroveRemoveError};
pub use grove_list::{GroveListOperation, GroveListError};
pub use init::{InitOperation, InitError};
