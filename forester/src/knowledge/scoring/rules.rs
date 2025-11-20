//! Score boosting rules for search results
//!
//! Defines declarative rules for boosting search result scores based on
//! file characteristics. Documentation files are prioritized over source code.

use bon::Builder;
use nutype::nutype;
use serde::{Deserialize, Serialize};

use crate::knowledge::domain::{Chunk, ForestRelativePath};

/// A multiplier applied to search scores
#[nutype(
    validate(greater_or_equal = 0.1, less_or_equal = 10.0),
    derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        PartialOrd,
        Serialize,
        Deserialize,
        AsRef,
        TryFrom,
        Into
    )
)]
pub struct BoostMultiplier(f32);

impl Default for BoostMultiplier {
    fn default() -> Self {
        Self::try_new(1.0).expect("1.0 is valid multiplier")
    }
}

/// A rule for boosting search scores based on file characteristics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct BoostRule {
    /// Human-readable description of the rule
    pub description: String,
    /// The pattern to match against
    pub pattern: BoostPattern,
    /// The multiplier to apply when matched
    pub multiplier: BoostMultiplier,
}

impl BoostRule {
    /// Check if this rule matches the given chunk
    pub fn matches(&self, chunk: &Chunk) -> bool {
        self.pattern.matches(&chunk.source.file_path)
    }
}

/// Pattern for matching files
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoostPattern {
    /// Match files with a specific extension (e.g., "md", "rs")
    Extension(String),
    /// Match files whose name matches exactly (e.g., "README.md")
    FileName(String),
    /// Match files whose path starts with a prefix (e.g., "docs/")
    PathPrefix(String),
}

impl BoostPattern {
    fn matches(&self, path: &ForestRelativePath) -> bool {
        let path_str = path.to_string();
        match self {
            BoostPattern::Extension(ext) => path_str.ends_with(&format!(".{}", ext)),
            BoostPattern::FileName(name) => {
                path_str.ends_with(&format!("/{}", name)) || path_str == *name
            }
            BoostPattern::PathPrefix(prefix) => path_str.starts_with(prefix),
        }
    }
}

/// Default boost rules prioritizing documentation over source code
pub fn default_boost_rules() -> Vec<BoostRule> {
    vec![
        BoostRule::builder()
            .description("README files (highest priority documentation)")
            .pattern(BoostPattern::FileName("README.md".to_string()))
            .multiplier(BoostMultiplier::try_new(1.3).unwrap())
            .build(),
        BoostRule::builder()
            .description("Documentation directory")
            .pattern(BoostPattern::PathPrefix("docs/".to_string()))
            .multiplier(BoostMultiplier::try_new(1.2).unwrap())
            .build(),
        BoostRule::builder()
            .description("Markdown documentation files")
            .pattern(BoostPattern::Extension("md".to_string()))
            .multiplier(BoostMultiplier::try_new(1.2).unwrap())
            .build(),
        BoostRule::builder()
            .description("Rust source with rustdoc comments")
            .pattern(BoostPattern::Extension("rs".to_string()))
            .multiplier(BoostMultiplier::try_new(1.1).unwrap())
            .build(),
    ]
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, LineCount, LineNumber, LineRange,
        MarkdownContext, RepoName, TokenCount,
    };

    fn create_test_chunk(file_path: &str) -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new("test").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("test content")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    #[test]
    fn test_boost_multiplier_validates_range() {
        // Given Valid multiplier values
        // When Creating BoostMultiplier
        // Then Valid values should succeed
        assert!(BoostMultiplier::try_new(0.1).is_ok());
        assert!(BoostMultiplier::try_new(1.0).is_ok());
        assert!(BoostMultiplier::try_new(10.0).is_ok());

        // And invalid values should fail
        assert!(BoostMultiplier::try_new(0.0).is_err());
        assert!(BoostMultiplier::try_new(10.1).is_err());
    }

    #[test]
    fn test_boost_multiplier_default() {
        // Given Default multiplier
        let multiplier = BoostMultiplier::default();

        // Then It should be 1.0 (no boost)
        assert_eq!(multiplier.into_inner(), 1.0);
    }

    #[test]
    fn test_boost_pattern_extension_matches() {
        // Given An extension pattern
        let pattern = BoostPattern::Extension("md".to_string());
        let chunk = create_test_chunk("docs/guide.md");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should match
        assert!(matches);
    }

    #[test]
    fn test_boost_pattern_extension_no_match() {
        // Given An extension pattern
        let pattern = BoostPattern::Extension("md".to_string());
        let chunk = create_test_chunk("src/main.rs");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should not match
        assert!(!matches);
    }

    #[test]
    fn test_boost_pattern_filename_matches() {
        // Given A filename pattern
        let pattern = BoostPattern::FileName("README.md".to_string());
        let chunk = create_test_chunk("docs/README.md");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should match
        assert!(matches);
    }

    #[test]
    fn test_boost_pattern_filename_matches_root() {
        // Given A filename pattern
        let pattern = BoostPattern::FileName("README.md".to_string());
        let chunk = create_test_chunk("README.md");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should match
        assert!(matches);
    }

    #[test]
    fn test_boost_pattern_filename_no_match() {
        // Given A filename pattern
        let pattern = BoostPattern::FileName("README.md".to_string());
        let chunk = create_test_chunk("docs/guide.md");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should not match
        assert!(!matches);
    }

    #[test]
    fn test_boost_pattern_path_prefix_matches() {
        // Given A path prefix pattern
        let pattern = BoostPattern::PathPrefix("docs/".to_string());
        let chunk = create_test_chunk("docs/architecture.md");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should match
        assert!(matches);
    }

    #[test]
    fn test_boost_pattern_path_prefix_no_match() {
        // Given A path prefix pattern
        let pattern = BoostPattern::PathPrefix("docs/".to_string());
        let chunk = create_test_chunk("src/main.rs");

        // When Checking if it matches
        let matches = pattern.matches(&chunk.source.file_path);

        // Then It should not match
        assert!(!matches);
    }

    #[test]
    fn test_boost_rule_matches() {
        // Given A boost rule
        let rule = BoostRule::builder()
            .description("Markdown files")
            .pattern(BoostPattern::Extension("md".to_string()))
            .multiplier(BoostMultiplier::try_new(1.2).unwrap())
            .build();
        let chunk = create_test_chunk("docs/guide.md");

        // When Checking if it matches
        let matches = rule.matches(&chunk);

        // Then It should match
        assert!(matches);
    }

    #[test]
    fn test_boost_rule_no_match() {
        // Given A boost rule
        let rule = BoostRule::builder()
            .description("Markdown files")
            .pattern(BoostPattern::Extension("md".to_string()))
            .multiplier(BoostMultiplier::try_new(1.2).unwrap())
            .build();
        let chunk = create_test_chunk("src/main.rs");

        // When Checking if it matches
        let matches = rule.matches(&chunk);

        // Then It should not match
        assert!(!matches);
    }

    #[test]
    fn test_default_boost_rules_exist() {
        // Given Default boost rules
        let rules = default_boost_rules();

        // Then There should be multiple rules
        assert!(!rules.is_empty());

        // And they should have descriptions
        for rule in &rules {
            assert!(!rule.description.is_empty());
        }
    }

    #[test]
    fn test_default_boost_rules_prioritize_readme() {
        // Given Default boost rules
        let rules = default_boost_rules();
        let readme_chunk = create_test_chunk("README.md");
        let md_chunk = create_test_chunk("guide.md");

        // When Finding matching rules
        let readme_multiplier = rules
            .iter()
            .find(|r| r.matches(&readme_chunk))
            .map(|r| r.multiplier.into_inner())
            .unwrap_or(1.0);

        let md_multiplier = rules
            .iter()
            .find(|r| r.matches(&md_chunk))
            .map(|r| r.multiplier.into_inner())
            .unwrap_or(1.0);

        // Then README should have higher boost than regular markdown
        assert!(readme_multiplier > md_multiplier);
    }

    #[test]
    fn test_default_boost_rules_prioritize_docs_over_source() {
        // Given Default boost rules
        let rules = default_boost_rules();
        let doc_chunk = create_test_chunk("docs/guide.md");
        let src_chunk = create_test_chunk("src/main.rs");

        // When Finding matching rules
        let doc_multiplier = rules
            .iter()
            .find(|r| r.matches(&doc_chunk))
            .map(|r| r.multiplier.into_inner())
            .unwrap_or(1.0);

        let src_multiplier = rules
            .iter()
            .find(|r| r.matches(&src_chunk))
            .map(|r| r.multiplier.into_inner())
            .unwrap_or(1.0);

        // Then Documentation should have higher boost than source
        assert!(doc_multiplier > src_multiplier);
    }
}
