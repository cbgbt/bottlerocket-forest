//! Applies boost rules to search result scores
//!
//! Prioritizes certain types of content (e.g., documentation over source code)
//! by multiplying relevance scores based on file characteristics.

use snafu::ResultExt;

use crate::knowledge::domain::{Chunk, RelevanceScore};

use super::rules::{BoostMultiplier, BoostRule};

/// Applies boost rules to search result scores
#[derive(Debug, Clone)]
pub struct ScoreBooster {
    rules: Vec<BoostRule>,
}

impl ScoreBooster {
    /// Create a score booster with the given rules
    pub fn new(rules: Vec<BoostRule>) -> Self {
        Self { rules }
    }

    /// Calculate the boost multiplier for a chunk
    ///
    /// Uses the multiplier from the first matching rule, or 1.0 if no rules match.
    pub fn calculate_boost(&self, chunk: &Chunk) -> BoostMultiplier {
        self.rules
            .iter()
            .find(|rule| rule.matches(chunk))
            .map(|rule| rule.multiplier)
            .unwrap_or_default()
    }

    /// Apply boost to a relevance score
    ///
    /// Multiplies the score by the boost multiplier and clamps to [0.0, 1.0].
    pub fn apply_boost(
        &self,
        score: RelevanceScore,
        chunk: &Chunk,
    ) -> Result<RelevanceScore, RelevanceScoreError> {
        use relevance_score_error::*;

        let multiplier = self.calculate_boost(chunk);
        let boosted = score.into_inner() * multiplier.into_inner();
        let clamped = boosted.clamp(0.0, 1.0);

        RelevanceScore::try_new(clamped)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(InvalidScoreSnafu { score: clamped })
    }
}

impl Default for ScoreBooster {
    fn default() -> Self {
        Self::new(super::rules::default_boost_rules())
    }
}

/// Errors that can occur when applying score boosts
#[derive(Debug, snafu::Snafu)]
#[snafu(module)]
pub enum RelevanceScoreError {
    #[snafu(display("Invalid relevance score: {score}"))]
    InvalidScore {
        score: f32,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkHash, ChunkId, ChunkSource, FileHash,
        ForestRelativePath, MarkdownContext, RepoName, TokenCount,
    };
    use crate::knowledge::scoring::rules::{BoostPattern, BoostRule};

    fn create_test_chunk(file_path: &str) -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(ChunkHash::new([0u8; 32]))
            .file_hash(FileHash::new([0u8; 32]))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new(file_path).unwrap())
                    .repo_name(RepoName::try_new("test").unwrap())
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
    fn test_score_booster_default_uses_default_rules() {
        // Given A default score booster
        let booster = ScoreBooster::default();

        // Then It should have rules
        assert!(!booster.rules.is_empty());
    }

    #[test]
    fn test_calculate_boost_returns_matching_multiplier() {
        // Given A booster with a specific rule
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("docs/guide.md");

        // When Calculating boost
        let multiplier = booster.calculate_boost(&chunk);

        // Then It should return the rule's multiplier
        assert_eq!(multiplier.into_inner(), 1.5);
    }

    #[test]
    fn test_calculate_boost_returns_default_when_no_match() {
        // Given A booster with rules that don't match
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("src/main.rs");

        // When Calculating boost
        let multiplier = booster.calculate_boost(&chunk);

        // Then It should return default (1.0)
        assert_eq!(multiplier.into_inner(), 1.0);
    }

    #[test]
    fn test_calculate_boost_uses_first_matching_rule() {
        // Given A booster with multiple matching rules
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.2).unwrap())
                .build(),
            BoostRule::builder()
                .description("Docs directory")
                .pattern(BoostPattern::new("docs/**").unwrap())
                .multiplier(BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("docs/guide.md");

        // When Calculating boost
        let multiplier = booster.calculate_boost(&chunk);

        // Then It should use the first matching rule
        assert_eq!(multiplier.into_inner(), 1.2);
    }

    #[test]
    fn test_apply_boost_multiplies_score() {
        // Given A booster and a score
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(2.0).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("guide.md");
        let score = RelevanceScore::try_new(0.5).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be multiplied (but clamped to 1.0)
        assert_eq!(boosted.into_inner(), 1.0);
    }

    #[test]
    fn test_apply_boost_clamps_to_one() {
        // Given A booster that would boost above 1.0
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(2.0).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("guide.md");
        let score = RelevanceScore::try_new(0.8).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be clamped to 1.0
        assert_eq!(boosted.into_inner(), 1.0);
    }

    #[test]
    fn test_apply_boost_preserves_score_when_no_match() {
        // Given A booster with non-matching rules
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("src/main.rs");
        let score = RelevanceScore::try_new(0.7).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be unchanged (multiplied by 1.0)
        assert_eq!(boosted.into_inner(), 0.7);
    }

    #[test]
    fn test_apply_boost_with_readme() {
        // Given A booster with README rule
        let rules = vec![
            BoostRule::builder()
                .description("README files")
                .pattern(BoostPattern::new("**/README.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.3).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("README.md");
        let score = RelevanceScore::try_new(0.5).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be boosted
        assert_eq!(boosted.into_inner(), 0.65);
    }

    #[test]
    fn test_apply_boost_with_low_score() {
        // Given A low score
        let rules = vec![
            BoostRule::builder()
                .description("Markdown files")
                .pattern(BoostPattern::new("**/*.md").unwrap())
                .multiplier(BoostMultiplier::try_new(1.5).unwrap())
                .build(),
        ];
        let booster = ScoreBooster::new(rules);
        let chunk = create_test_chunk("guide.md");
        let score = RelevanceScore::try_new(0.1).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be boosted proportionally
        assert_eq!(boosted.into_inner(), 0.15);
    }

    #[test]
    fn test_apply_boost_with_empty_rules() {
        // Given A booster with no rules
        let booster = ScoreBooster::new(vec![]);
        let chunk = create_test_chunk("guide.md");
        let score = RelevanceScore::try_new(0.7).unwrap();

        // When Applying boost
        let boosted = booster.apply_boost(score, &chunk).unwrap();

        // Then Score should be unchanged
        assert_eq!(boosted.into_inner(), 0.7);
    }
}
