//! Git abstraction layer for forester.
//!
//! Provides operations for bare repositories and clones.

mod bare;

pub use bare::{BareRepository, CheckoutError, CloneBareError, CloneToError};
