//! Hook execution phases.

/// Lifecycle phases where hooks can execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookPhase {
    /// Before seeding the forest.
    PreSeed,
    /// After seeding the forest.
    PostSeed,
    /// Before creating a grove.
    PreGroveCreate,
    /// After creating a grove.
    PostGroveCreate,
    /// Before removing a grove.
    PreGroveRemove,
    /// After removing a grove.
    PostGroveRemove,
}
