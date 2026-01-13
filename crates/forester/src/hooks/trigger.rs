//! Hook execution triggers.

use std::fmt;

/// Events that can trigger hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// After seeding the forest.
    PostSeed,
    /// After creating a grove.
    PostGroveCreate,
    /// After removing a grove.
    PostGroveRemove,
}

impl Trigger {
    /// Parses a trigger from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "post-seed" => Some(Self::PostSeed),
            "post-grove-create" => Some(Self::PostGroveCreate),
            "post-grove-remove" => Some(Self::PostGroveRemove),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PostSeed => "post-seed",
            Self::PostGroveCreate => "post-grove-create",
            Self::PostGroveRemove => "post-grove-remove",
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
