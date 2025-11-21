//! Score boosting for search results
//!
//! This module provides declarative rules for boosting search result scores
//! based on file characteristics. Documentation files are prioritized over
//! source code to improve search relevance.
//!
//! # Example
//!
//! ```no_run
//! use forester::knowledge::scoring::{ScoreBooster, default_boost_rules};
//! use forester::knowledge::domain::RelevanceScore;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use forester::knowledge::domain::*;
//! # let chunk = Chunk::builder()
//! #     .id(ChunkId::new(uuid::Uuid::new_v4()))
//! #     .source(ChunkSource::builder()
//! #         .file_path(ForestRelativePath::try_new("README.md")?)
//! #         .repo_name(RepoName::try_new("test")?)
//! #         .build())
//! #     .content(ChunkContent::builder()
//! #         .text("test")
//! #         .token_count(TokenCount::try_new(10)?)
//! #         .build())
//! #     .context(ChunkContext::Markdown(MarkdownContext::builder()
//! #         .heading_hierarchy(vec![])
//! #         .build()))
//! #     .build();
//! let booster = ScoreBooster::default();
//! let score = RelevanceScore::try_new(0.8)?;
//! let boosted = booster.apply_boost(score, &chunk)?;
//! # Ok(())
//! # }
//! ```

mod booster;
mod rules;

pub use booster::{RelevanceScoreError, ScoreBooster};
pub use rules::{BoostMultiplier, BoostPattern, BoostRule, default_boost_rules};
