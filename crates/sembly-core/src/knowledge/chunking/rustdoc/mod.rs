//! Extracts and chunks Rust documentation comments for semantic search.
//!
//! Implements chunking for Rust source files by extracting doc comments
//! (`///` and `//!`) while ignoring inline comments (`//`). Uses `syn` for parsing
//! and `text-splitter` for token-aware chunking with overlap.
//!
//! Doc comments are extracted from:
//! - Module-level comments (`//!`)
//! - Functions, structs, enums, traits
//! - Methods in impl blocks
//!
//! Each chunk preserves metadata including item name, visibility, and function signatures.
//!
//! ## Module Organization
//!
//! - `extraction` - Doc comment extraction logic for different Rust item types

mod extraction;

use std::path::Path;
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;

use super::{ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::EmbeddingModelConfig;
use crate::knowledge::domain::{Chunk, ItemName, Visibility};

pub(crate) use extraction::DocExtractor;

/// Extracts and chunks Rust documentation comments.
///
/// Uses `syn` to parse Rust syntax and extract `///` and `//!` doc comments,
/// then uses `text-splitter` to chunk long comments with token-based overlap.
pub struct RustDocChunker {
    splitter: TextSplitter<Tokenizer>,
    tokenizer: Tokenizer,
    filter: Option<crate::knowledge::indexing::RustFilter>,
}

impl RustDocChunker {
    /// Initializes chunker with tokenizer and splitter configured for the embedding model.
    pub fn from_config(config: &EmbeddingModelConfig) -> Result<Self, ChunkingError> {
        Self::from_config_with_filter(config, None)
    }

    /// Initializes chunker with tokenizer, splitter, and filtering rules for Rust items.
    pub fn from_config_with_filter(
        config: &EmbeddingModelConfig,
        filter: Option<crate::knowledge::indexing::RustFilter>,
    ) -> Result<Self, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let tokenizer = Tokenizer::from_pretrained(&config.model_name, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(TokenizerInitSnafu)?;

        let tokenizer_for_counting = Tokenizer::from_pretrained(&config.model_name, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(TokenizerInitSnafu)?;

        let splitter = TextSplitter::new(
            ChunkConfig::new(config.max_tokens)
                .with_sizer(tokenizer)
                .with_overlap(config.overlap_tokens)
                .expect("overlap configuration should be valid"),
        );

        Ok(Self {
            splitter,
            tokenizer: tokenizer_for_counting,
            filter,
        })
    }
}

impl ChunkingStrategy for RustDocChunker {
    fn supports(&self, file_path: &Path) -> bool {
        file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "rs")
            .unwrap_or(false)
    }

    fn chunk(&self, input: &ChunkingInput) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;
        use syn::File;

        let content = input.content.as_ref();
        let file_path = input.source.file_path.to_string();

        let syntax_tree: File = syn::parse_str(content)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseSnafu {
                file_path: file_path.clone(),
            })?;

        let extractor = DocExtractor {
            splitter: &self.splitter,
            tokenizer: &self.tokenizer,
            filter: self.filter.as_ref(),
        };

        let mut chunks = Vec::new();

        let module_doc = DocExtractor::extract_doc_text(&syntax_tree.attrs);
        if !module_doc.trim().is_empty() {
            chunks.extend(
                extractor.create_chunks(
                    &module_doc,
                    ItemName::try_new("module")
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?,
                    Visibility::Public,
                    None,
                    crate::knowledge::indexing::RustItemType::Module,
                    input,
                )?,
            );
        }

        for item in &syntax_tree.items {
            chunks.extend(extractor.extract_item_doc_chunks(item, input)?);
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::chunking::ChunkingStrategy;
    use crate::knowledge::domain::EmbeddingModelConfig;
    use crate::knowledge::domain::{
        ChunkContext, ChunkSource, ChunkableContent, FileHash, ForestRelativePath, ItemName,
        RepoName,
    };
    use test_case::test_case;

    fn test_config() -> EmbeddingModelConfig {
        EmbeddingModelConfig::default()
    }

    fn create_test_input(content: &str) -> ChunkingInput {
        ChunkingInput {
            content: ChunkableContent::new(content.to_string()),
            source: ChunkSource::builder()
                .file_path(ForestRelativePath::try_new("test.rs").unwrap())
                .repo_name(RepoName::try_new("test-repo").unwrap())
                .build(),
            file_hash: FileHash::new([0u8; 32]),
        }
    }

    #[test_case(r#"
/// Processes data according to specified parameters
pub fn process_data(input: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    Ok(input.to_vec())
}
"#, "Processes data" ; "fn_item")]
    #[test_case(r#"
/// Configuration for the application
pub struct Config {
    pub api_key: String,
}
"#, "Configuration" ; "struct_item")]
    #[test_case(r#"
//! This module contains utilities for parsing configuration files.

pub fn parse_config() {}
"#, "utilities for parsing" ; "module_item")]
    #[test_case(r#"
/// Repository for data persistence
pub trait Repository {
    fn save(&self);
}
"#, "Repository for data" ; "trait_item")]
    fn test_extracts_doc_comments_for_item_types(content: &str, expected_text: &str) {
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert!(!chunks.is_empty());
        assert!(chunks[0].content.text.contains(expected_text));
    }

    #[test]
    fn test_ignores_items_without_doc_comments() {
        let content = r#"
pub fn no_docs() {}

// Regular comment
pub struct NoDocs {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_empty_rust_file() {
        let input = create_test_input("");
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_invalid_rust_syntax() {
        let content = "pub fn incomplete(";
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let result = chunker.chunk(&input);

        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_items_create_separate_chunks() {
        let content = r#"
/// First function
pub fn first() {}

/// Second function
pub fn second() {}

/// Third function
pub fn third() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert_eq!(chunks.len(), 3);

        let names: Vec<_> = chunks
            .iter()
            .filter_map(|c| {
                if let ChunkContext::RustDoc(ctx) = &c.context {
                    Some(ctx.item_name.clone())
                } else {
                    None
                }
            })
            .collect();

        assert!(names.contains(&ItemName::try_new("first").unwrap()));
        assert!(names.contains(&ItemName::try_new("second").unwrap()));
        assert!(names.contains(&ItemName::try_new("third").unwrap()));
    }
}
