//! Operations layer for forester.
//!
//! Each operation takes references to EventEmitter and HookRegistry,
//! uses domain types, and emits events for user feedback.

mod grove_create;
mod grove_list;
mod grove_remove;
mod init;
mod seed;

pub use grove_create::{GroveCreateError, GroveCreateOperation};
pub use grove_list::{GroveListError, GroveListOperation};
pub use grove_remove::{GroveRemoveError, GroveRemoveOperation};
pub use init::{InitError, InitOperation};
pub use seed::{SeedError, SeedOperation};
