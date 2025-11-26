//! Routes files to appropriate chunking strategies based on file type.

use snafu::{ResultExt, Snafu};
use std::path::Path;

use super::{ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::{Chunk, EmbeddingModelConfig};
use crate::knowledge::indexing::IndexingFilter;

/// Routes files to appropriate chunking strategies.
pub struct ChunkingDispatcher {
    strategies: Vec<Box<dyn ChunkingStrategy>>,
}

impl ChunkingDispatcher {
    /// Initializes dispatcher with markdown and Rust file strategies.
    #[must_use = "dispatcher must be used or initialization error handled"]
    pub fn with_defaults(config: &EmbeddingModelConfig) -> Result<Self, DispatchError> {
        Self::with_defaults_and_filter(config, &IndexingFilter::default())
    }

    /// Initializes dispatcher with markdown and Rust file strategies and applies filtering rules.
    #[must_use = "dispatcher must be used or initialization error handled"]
    pub fn with_defaults_and_filter(
        config: &EmbeddingModelConfig,
        filter: &IndexingFilter,
    ) -> Result<Self, DispatchError> {
        use dispatch_error::*;

        let markdown_chunker = super::markdown::MarkdownChunker::from_config(config)
            .context(StrategyInitFailedSnafu)?;
        let rustdoc_chunker = super::rustdoc::RustDocChunker::from_config_with_filter(
            config,
            filter.rust_filter().cloned(),
        )
        .context(StrategyInitFailedSnafu)?;

        Ok(Self {
            strategies: vec![Box::new(markdown_chunker), Box::new(rustdoc_chunker)],
        })
    }

    /// Chunks a file using the appropriate strategy.
    ///
    /// Returns `None` if no strategy supports the file type.
    pub fn chunk_file(&self, input: &ChunkingInput) -> Option<Result<Vec<Chunk>, DispatchError>> {
        use dispatch_error::*;

        let file_path_str = input.source.file_path.to_string();
        let file_path = Path::new(&file_path_str);

        self.strategies
            .iter()
            .find(|strategy| strategy.supports(file_path))
            .map(|strategy| strategy.chunk(input).context(ChunkingFailedSnafu))
    }
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module, visibility(pub(crate)))]
pub enum DispatchError {
    #[snafu(display("No chunking strategy available for this file type"))]
    #[diagnostic(
        code(sembly::chunking::no_strategy_found),
        help("Only Markdown (.md) and Rust (.rs) files are currently supported")
    )]
    NoStrategyFound,

    #[snafu(display("Failed to chunk file content into searchable segments"))]
    #[diagnostic(
        code(sembly::chunking::chunking_failed),
        help("The file may contain invalid syntax or exceed size limits")
    )]
    ChunkingFailed { source: ChunkingError },

    #[snafu(display("Failed to initialize chunking strategy"))]
    #[diagnostic(
        code(sembly::chunking::strategy_init_failed),
        help("Check that the embedding model configuration is valid")
    )]
    StrategyInitFailed { source: ChunkingError },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::chunking::strategy::MockChunkingStrategy;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkHash, ChunkId, ChunkSource, ChunkableContent, FileHash,
        ForestRelativePath, MarkdownContext, RepoName, TokenCount,
    };
    use test_case::test_case;

    fn create_test_input(file_path: &str) -> ChunkingInput {
        ChunkingInput {
            content: ChunkableContent::new("# Test\nContent".to_string()),
            source: ChunkSource::builder()
                .file_path(ForestRelativePath::try_new(file_path).unwrap())
                .repo_name(RepoName::try_new("test-repo").unwrap())
                .build(),
            file_hash: FileHash::new([0u8; 32]),
        }
    }

    fn create_test_chunk() -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(ChunkHash::new([0u8; 32]))
            .file_hash(FileHash::new([0u8; 32]))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("Test content".to_string())
                    .token_count(TokenCount::try_new(2).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    #[test]
    fn test_with_defaults_creates_dispatcher() {
        // Given An embedding model config
        let config = EmbeddingModelConfig::default();

        // When Creating a dispatcher with defaults
        let result = ChunkingDispatcher::with_defaults(&config);

        // Then The dispatcher should be created successfully
        assert!(result.is_ok());
    }

    #[test_case("test.md" ; "markdown file")]
    #[test_case("test.rs" ; "rust file")]
    fn test_with_defaults_supports_file_types(file_path: &str) {
        // Given A dispatcher with default strategies
        let config = EmbeddingModelConfig::default();
        let dispatcher = ChunkingDispatcher::with_defaults(&config).unwrap();
        let input = create_test_input(file_path);

        // When Chunking a supported file
        let result = dispatcher.chunk_file(&input);

        // Then A result should be returned (not None)
        assert!(result.is_some());
    }

    #[test]
    fn test_chunk_file_returns_none_for_unsupported_type() {
        // Given A dispatcher with default strategies
        let config = EmbeddingModelConfig::default();
        let dispatcher = ChunkingDispatcher::with_defaults(&config).unwrap();
        let input = create_test_input("test.txt");

        // When Chunking an unsupported file type
        let result = dispatcher.chunk_file(&input);

        // Then None should be returned
        assert!(result.is_none());
    }

    #[test]
    fn test_chunk_file_routes_to_correct_strategy() {
        // Given A dispatcher with a mocked strategy
        let mut mock_strategy = MockChunkingStrategy::new();
        mock_strategy
            .expect_supports()
            .returning(|path| path.extension().and_then(|s| s.to_str()) == Some("md"));
        mock_strategy
            .expect_chunk()
            .times(1)
            .returning(|_| Ok(vec![create_test_chunk()]));

        let dispatcher = ChunkingDispatcher {
            strategies: vec![Box::new(mock_strategy)],
        };

        let input = create_test_input("test.md");

        // When Chunking a file
        let result = dispatcher.chunk_file(&input);

        // Then The strategy should be called and return chunks
        assert!(result.is_some());
        let chunks = result.unwrap().unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_file_propagates_chunking_error() {
        // Given A dispatcher with a strategy that returns an error
        let mut mock_strategy = MockChunkingStrategy::new();
        mock_strategy.expect_supports().returning(|_| true);
        mock_strategy.expect_chunk().times(1).returning(|_| {
            Err(ChunkingError::TokenLimitExceeded {
                actual: 1000,
                max: 256,
            })
        });

        let dispatcher = ChunkingDispatcher {
            strategies: vec![Box::new(mock_strategy)],
        };

        let input = create_test_input("test.md");

        // When Chunking a file
        let result = dispatcher.chunk_file(&input);

        // Then An error should be returned
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap(),
            Err(DispatchError::ChunkingFailed { .. })
        ));
    }

    #[test]
    fn test_chunk_file_tries_strategies_in_order() {
        // Given A dispatcher with multiple strategies
        let mut mock_strategy1 = MockChunkingStrategy::new();
        mock_strategy1.expect_supports().returning(|_| false);

        let mut mock_strategy2 = MockChunkingStrategy::new();
        mock_strategy2.expect_supports().returning(|_| true);
        mock_strategy2
            .expect_chunk()
            .times(1)
            .returning(|_| Ok(vec![create_test_chunk()]));

        let dispatcher = ChunkingDispatcher {
            strategies: vec![Box::new(mock_strategy1), Box::new(mock_strategy2)],
        };

        let input = create_test_input("test.md");

        // When Chunking a file
        let result = dispatcher.chunk_file(&input);

        // Then The second strategy should be used
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }
}
