//! Extracts and chunks Rust documentation comments for semantic search.
//!
//! This module implements chunking for Rust source files by extracting doc comments
//! (`///` and `//!`) while ignoring inline comments (`//`). It uses `syn` for parsing
//! and `text-splitter` for token-aware chunking with overlap.
//!
//! Doc comments are extracted from:
//! - Module-level comments (`//!`)
//! - Functions, structs, enums, traits
//! - Methods in impl blocks
//!
//! Each chunk preserves metadata including item name, visibility, and function signatures.

use std::path::Path;
use syn::{Attribute, File, Item, ItemImpl};
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;

use super::{ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, IndexData, ItemName, RustDocContext, Signature,
    TokenCount, Visibility,
};
use crate::knowledge::storage::EmbeddingModelConfig;

/// Extracts and chunks Rust documentation comments.
///
/// Uses `syn` to parse Rust syntax and extract `///` and `//!` doc comments,
/// then uses `text-splitter` to chunk long comments with token-based overlap.
pub struct RustDocChunker {
    splitter: TextSplitter<Tokenizer>,
    tokenizer: Tokenizer,
}

impl RustDocChunker {
    /// Creates a chunker configured with the specified embedding model parameters.
    ///
    /// Initializes tokenizers and text splitter with token limits and overlap from config.
    pub fn from_config(config: &EmbeddingModelConfig) -> Result<Self, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let tokenizer = Tokenizer::from_pretrained(&config.model_name, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseSnafu)?;

        let tokenizer_for_counting = Tokenizer::from_pretrained(&config.model_name, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseSnafu)?;

        let splitter = TextSplitter::new(
            ChunkConfig::new(config.max_tokens)
                .with_sizer(tokenizer)
                .with_overlap(config.overlap_tokens)
                .expect("overlap configuration should be valid"),
        );

        Ok(Self {
            splitter,
            tokenizer: tokenizer_for_counting,
        })
    }

    /// Extracts doc comment text from attributes.
    ///
    /// Collects `///` and `//!` comments, ignoring inline `//` comments.
    fn extract_doc_text(attrs: &[Attribute]) -> String {
        attrs
            .iter()
            .filter_map(|attr| {
                if attr.path().is_ident("doc")
                    && let syn::Meta::NameValue(meta) = &attr.meta
                    && let syn::Expr::Lit(expr_lit) = &meta.value
                    && let syn::Lit::Str(lit_str) = &expr_lit.lit
                {
                    return Some(lit_str.value());
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extracts doc comments from a syntax tree item and creates chunks.
    ///
    /// Handles functions, structs, enums, traits, modules, and impl blocks.
    /// Recursively processes nested items in modules.
    fn extract_item_doc_chunks(
        &self,
        item: &Item,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let mut chunks = Vec::new();

        match item {
            Item::Fn(item_fn) => {
                let doc_text = Self::extract_doc_text(&item_fn.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_fn.sig.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    let sig = &item_fn.sig;
                    let signature = Signature::try_new(quote::quote!(#sig).to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_fn.vis.clone()),
                        Some(signature),
                        input,
                    )?);
                }
            }
            Item::Struct(item_struct) => {
                let doc_text = Self::extract_doc_text(&item_struct.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_struct.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_struct.vis.clone()),
                        None,
                        input,
                    )?);
                }
            }
            Item::Enum(item_enum) => {
                let doc_text = Self::extract_doc_text(&item_enum.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_enum.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_enum.vis.clone()),
                        None,
                        input,
                    )?);
                }
            }
            Item::Trait(item_trait) => {
                let doc_text = Self::extract_doc_text(&item_trait.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_trait.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_trait.vis.clone()),
                        None,
                        input,
                    )?);
                }
            }
            Item::Mod(item_mod) => {
                let doc_text = Self::extract_doc_text(&item_mod.attrs);
                let mod_name = ItemName::try_new(item_mod.ident.to_string())
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(ParseSnafu)?;

                if !doc_text.trim().is_empty() {
                    chunks.extend(self.create_chunks(
                        &doc_text,
                        mod_name,
                        Visibility::from(item_mod.vis.clone()),
                        None,
                        input,
                    )?);
                }

                if let Some((_, items)) = &item_mod.content {
                    for item in items {
                        chunks.extend(self.extract_item_doc_chunks(item, input)?);
                    }
                }
            }
            Item::Impl(item_impl) => {
                chunks.extend(self.extract_impl_method_chunks(item_impl, input)?);
            }
            _ => {}
        }

        Ok(chunks)
    }

    /// Extracts doc comments from methods in impl blocks and creates chunks.
    fn extract_impl_method_chunks(
        &self,
        item_impl: &ItemImpl,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let mut chunks = Vec::new();

        for impl_item in &item_impl.items {
            if let syn::ImplItem::Fn(method) = impl_item {
                let doc_text = Self::extract_doc_text(&method.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(method.sig.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    let sig = &method.sig;
                    let signature = Signature::try_new(quote::quote!(#sig).to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(method.vis.clone()),
                        Some(signature),
                        input,
                    )?);
                }
            }
        }

        Ok(chunks)
    }

    /// Splits doc text into chunks with token-based overlap.
    ///
    /// Each chunk preserves the same metadata (item name, visibility, signature).
    fn create_chunks(
        &self,
        doc_text: &str,
        item_name: ItemName,
        visibility: Visibility,
        signature: Option<Signature>,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let text_chunks: Vec<&str> = self.splitter.chunks(doc_text).collect();

        text_chunks
            .into_iter()
            .map(|text| {
                let encoding = self.tokenizer.encode(text, false).context(ParseSnafu)?;

                let token_count = encoding.len().max(1);

                let token_count = TokenCount::try_new(token_count)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(ParseSnafu)?;

                let context = RustDocContext::builder()
                    .item_name(item_name.clone())
                    .visibility(visibility)
                    .maybe_signature(signature.clone())
                    .build();

                Ok(Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(input.source.clone())
                    .content(
                        ChunkContent::builder()
                            .text(text)
                            .token_count(token_count)
                            .build(),
                    )
                    .context(ChunkContext::RustDoc(context))
                    .indexed_at(std::time::SystemTime::now())
                    .index_data(IndexData::Fast {
                        bm25_terms: std::collections::BTreeMap::new(),
                    })
                    .build())
            })
            .collect()
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

        let content = input.content.as_ref();

        let syntax_tree: File = syn::parse_str(content)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseSnafu)?;

        let mut chunks = Vec::new();

        let module_doc = Self::extract_doc_text(&syntax_tree.attrs);
        if !module_doc.trim().is_empty() {
            chunks.extend(
                self.create_chunks(
                    &module_doc,
                    ItemName::try_new("module")
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu)?,
                    Visibility::Public,
                    None,
                    input,
                )?,
            );
        }

        for item in &syntax_tree.items {
            chunks.extend(self.extract_item_doc_chunks(item, input)?);
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkContext, ChunkSource, ChunkableContent, ForestRelativePath, ItemName, LineCount,
        LineNumber, LineRange, RepoName, Visibility,
    };
    use crate::knowledge::storage::EmbeddingModelConfig;
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
                .line_range(
                    LineRange::builder()
                        .start(LineNumber::try_new(1).unwrap())
                        .line_count(LineCount::try_new(10).unwrap())
                        .build(),
                )
                .build(),
        }
    }

    #[test_case("test.rs" => true ; "rust extension")]
    #[test_case("test.md" => false ; "markdown extension")]
    #[test_case("test.txt" => false ; "text extension")]
    #[test_case("test" => false ; "no extension")]
    fn test_file_extension_support(path: &str) -> bool {
        // Given A Rust doc chunker
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Checking file support
        // Then Return whether it's supported
        chunker.supports(Path::new(path))
    }

    #[test]
    fn test_extracts_function_doc_comments() {
        // Given Rust code with function doc comments
        let content = r#"
/// Processes data according to specified parameters
///
/// This function handles large datasets efficiently.
pub fn process_data(input: &[u8]) -> Result<Vec<u8>, Error> {
    // Regular comment - should be ignored
    Ok(input.to_vec())
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create chunk with doc comment text
        assert!(!chunks.is_empty());
        let chunk = &chunks[0];
        assert!(chunk.content.text.contains("Processes data"));
        assert!(chunk.content.text.contains("large datasets"));

        // And Should not include regular comments
        assert!(!chunk.content.text.contains("Regular comment"));
    }

    #[test]
    fn test_extracts_struct_doc_comments() {
        // Given Rust code with struct doc comments
        let content = r#"
/// Configuration for the application
///
/// Contains all settings needed to run the service.
pub struct Config {
    pub api_key: String,
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create chunk with struct doc comment
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.text.contains("Configuration"));
    }

    #[test]
    fn test_extracts_module_doc_comments() {
        // Given Rust code with module doc comments
        let content = r#"
//! This module contains utilities for parsing configuration files.
//!
//! It supports TOML and JSON formats.

pub fn parse_config() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create chunk with module doc comment
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.text.contains("utilities for parsing"));
    }

    #[test]
    fn test_captures_function_metadata() {
        // Given Rust code with a public function
        let content = r#"
/// Validates input data
pub fn validate(input: &str) -> bool {
    true
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture function metadata
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.item_name, ItemName::try_new("validate").unwrap());
            assert_eq!(ctx.visibility, Visibility::Public);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_struct_metadata() {
        // Given Rust code with a public struct
        let content = r#"
/// User profile data
pub struct User {
    name: String,
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture struct metadata
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.item_name, ItemName::try_new("User").unwrap());
            assert_eq!(ctx.visibility, Visibility::Public);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_enum_metadata() {
        // Given Rust code with an enum
        let content = r#"
/// Connection state
pub enum State {
    Connected,
    Disconnected,
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture enum metadata
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.item_name, ItemName::try_new("State").unwrap());
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_trait_metadata() {
        // Given Rust code with a trait
        let content = r#"
/// Repository for data persistence
pub trait Repository {
    fn save(&self);
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture trait metadata
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.item_name, ItemName::try_new("Repository").unwrap());
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_visibility_public() {
        // Given Rust code with public item
        let content = r#"
/// Public function
pub fn public_fn() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture public visibility
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.visibility, Visibility::Public);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_visibility_crate() {
        // Given Rust code with crate-visible item
        let content = r#"
/// Crate-visible function
pub(crate) fn crate_fn() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture crate visibility
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.visibility, Visibility::Crate);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_visibility_private() {
        // Given Rust code with private item
        let content = r#"
/// Private function
fn private_fn() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture private visibility
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.visibility, Visibility::Private);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_function_signature() {
        // Given Rust code with function signature
        let content = r#"
/// Processes data
pub fn process(input: &str, count: usize) -> Result<String, Error> {
    Ok(input.to_string())
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should capture signature
        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert!(ctx.signature.is_some());
            let sig = ctx.signature.as_ref().unwrap();
            assert!(sig.to_string().contains("process"));
            assert!(sig.to_string().contains("input"));
            assert!(sig.to_string().contains("count"));
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_respects_token_limit() {
        // Given Rust code with very long doc comment
        let long_doc = format!("/// {}\n", "word ".repeat(300));
        let content = format!("{}pub fn test() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then All chunks should respect token limit
        for chunk in &chunks {
            assert!(
                chunk.content.token_count.into_inner() <= 256,
                "Chunk exceeded token limit: {}",
                chunk.content.token_count.into_inner()
            );
        }
    }

    #[test]
    fn test_chunks_have_overlap() {
        // Given Rust code with long doc comment
        let long_doc = format!("/// {}\n", "This is sentence number X. ".repeat(200));
        let content = format!("{}pub fn test() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create multiple chunks with overlap
        if chunks.len() > 1 {
            let first_chunk_end = &chunks[0].content.text[chunks[0].content.text.len() - 50..];
            let second_chunk_start =
                &chunks[1].content.text[..50.min(chunks[1].content.text.len())];

            // And Some content should appear in both chunks
            assert!(
                first_chunk_end
                    .split_whitespace()
                    .any(|word| second_chunk_start.contains(word)),
                "Expected overlap between chunks"
            );
        }
    }

    #[test]
    fn test_preserves_metadata_across_chunks() {
        // Given Rust code with long doc comment that will be split
        let long_doc = format!("/// {}\n", "word ".repeat(300));
        let content = format!("{}pub fn long_function() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then All chunks should have same metadata
        if chunks.len() > 1 {
            let first_ctx = if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
                ctx
            } else {
                panic!("Expected RustDoc context");
            };

            for chunk in &chunks[1..] {
                if let ChunkContext::RustDoc(ctx) = &chunk.context {
                    assert_eq!(ctx.item_name, first_ctx.item_name);
                    assert_eq!(ctx.visibility, first_ctx.visibility);
                } else {
                    panic!("Expected RustDoc context");
                }
            }
        }
    }

    #[test]
    fn test_ignores_items_without_doc_comments() {
        // Given Rust code with items that have no doc comments
        let content = r#"
pub fn no_docs() {}

// Regular comment
pub struct NoDocs {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should return no chunks
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_empty_rust_file() {
        // Given Empty Rust file
        let input = create_test_input("");
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should return no chunks
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_invalid_rust_syntax() {
        // Given Invalid Rust syntax
        let content = "pub fn incomplete(";
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let result = chunker.chunk(&input);

        // Then Should return parse error
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_items_create_separate_chunks() {
        // Given Rust code with multiple documented items
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

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create separate chunks for each item
        assert_eq!(chunks.len(), 3);

        // And Each chunk should have different item name
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

    #[test]
    fn test_all_chunks_have_valid_token_counts() {
        // Given Rust code with various doc comments
        let content = r#"
/// Short doc
pub fn short() {}

/// Medium length documentation that spans multiple lines
/// and provides more detail about the function.
pub fn medium() {}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then All chunks should have valid token counts
        for chunk in &chunks {
            assert!(chunk.content.token_count.into_inner() > 0);
        }
    }
}
