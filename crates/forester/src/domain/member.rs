//! Member repository types for forest configuration.

use nutype::nutype;


/// Repository name within the forest.
#[nutype(validate(not_empty), derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize))]
pub struct MemberName(String);

/// Git remote URL for a member repository.
#[nutype(validate(not_empty), derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize))]
pub struct Remote(String);

/// Git branch name.
#[nutype(validate(not_empty), derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize))]
pub struct BranchName(String);
