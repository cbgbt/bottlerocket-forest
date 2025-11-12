//! Index mode selection for search strategy

use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::fmt;
use std::str::FromStr;

/// The indexing and search strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexMode {
    /// Fast keyword-based search using BM25
    Fast,
    /// Semantic search using embeddings
    Best,
}

impl fmt::Display for IndexMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl IndexMode {
    /// Returns the display name for user-facing output
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Best => "best",
        }
    }

    /// Returns the technical description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Fast => "BM25 keyword search",
            Self::Best => "Semantic search with embeddings",
        }
    }
}

impl FromStr for IndexMode {
    type Err = InvalidIndexMode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "best" => Ok(Self::Best),
            _ => Err(InvalidIndexMode {
                input: s.to_string(),
            }),
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Invalid index mode: '{input}'. Valid options: 'fast', 'best'"))]
pub struct InvalidIndexMode {
    input: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    #[test_case("fast", IndexMode::Fast ; "lowercase fast")]
    #[test_case("best", IndexMode::Best ; "lowercase best")]
    #[test_case("FAST", IndexMode::Fast ; "uppercase fast")]
    #[test_case("Best", IndexMode::Best ; "mixed case best")]
    #[test_case("FaSt", IndexMode::Fast ; "mixed case fast")]
    fn test_parse_valid(input: &str, expected: IndexMode) {
        // Given A valid mode string
        let result = input.parse::<IndexMode>();

        // When Parsing the mode
        // Then It should succeed with expected value
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_invalid() {
        // Given An invalid string
        let result = "invalid".parse::<IndexMode>();

        // When Parsing the mode
        // Then It should fail
        assert!(result.is_err());
    }

    #[test_case(IndexMode::Fast, "fast" ; "fast display name")]
    #[test_case(IndexMode::Best, "best" ; "best display name")]
    fn test_display_name(mode: IndexMode, expected: &str) {
        // Given An index mode
        // When Getting display name
        // Then It should match expected value
        assert_eq!(mode.display_name(), expected);
    }

    #[test_case(IndexMode::Fast, "BM25 keyword search" ; "fast description")]
    #[test_case(IndexMode::Best, "Semantic search with embeddings" ; "best description")]
    fn test_description(mode: IndexMode, expected: &str) {
        // Given An index mode
        // When Getting description
        // Then It should match expected value
        assert_eq!(mode.description(), expected);
    }
}
