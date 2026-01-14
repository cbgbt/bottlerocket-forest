//! Extracts and chunks Java documentation comments for semantic search.
//!
//! Implements chunking for Java source files by extracting Javadoc comments
//! (/** ... */ comments immediately preceding declarations). Uses `tree-sitter` for parsing
//! and `text-splitter` for token-aware chunking with overlap.
//!
//! Javadoc comments are extracted from:
//! - Classes, interfaces, enums, records
//! - Methods, constructors, fields
//! - Annotation type declarations
//!
//! Each chunk preserves metadata including item name, visibility, and signatures.

mod extraction;

use std::path::Path;
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;
use tree_sitter::Parser;

use self::extraction::{
    extract_doc_comment, extract_identifier, get_visibility, node_kinds, to_filter_type,
};
use super::{ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::EmbeddingModelConfig;
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkHash, DocLineCount, ItemName, JavaDocContext,
    JavaItemType, JavaVisibility, PackageName, Signature, TokenCount,
};
use crate::knowledge::indexing::JavaFilter;

/// Metadata for creating a chunk from a Java declaration.
struct ChunkMetadata {
    item_name: ItemName,
    visibility: JavaVisibility,
    signature: Option<Signature>,
    item_type: JavaItemType,
}

/// Context for processing Java nodes during chunking.
struct ProcessingContext<'a> {
    source_bytes: &'a [u8],
    package_name: &'a Option<PackageName>,
    input: &'a ChunkingInput,
}

/// Extracts and chunks Java documentation comments.
pub struct JavaDocChunker {
    splitter: TextSplitter<Tokenizer>,
    tokenizer: Tokenizer,
    filter: Option<JavaFilter>,
}

impl JavaDocChunker {
    /// Initializes chunker with tokenizer and splitter configured for the embedding model.
    pub fn from_config(config: &EmbeddingModelConfig) -> Result<Self, ChunkingError> {
        Self::from_config_with_filter(config, None)
    }

    /// Initializes chunker with tokenizer, splitter, and filtering rules for Java items.
    pub fn from_config_with_filter(
        config: &EmbeddingModelConfig,
        filter: Option<JavaFilter>,
    ) -> Result<Self, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let tokenizer = Tokenizer::from_pretrained(&config.model_name, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(TokenizerInitSnafu)?;

        let tokenizer_for_counting = tokenizer.clone();

        let splitter = TextSplitter::new(
            #[expect(clippy::expect_used)]
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

    fn should_index(
        &self,
        visibility: &crate::knowledge::domain::Visibility,
        item_type: JavaItemType,
        doc: &str,
    ) -> bool {
        let Some(ref filter) = self.filter else {
            return true;
        };
        let filter_type = to_filter_type(item_type);
        filter.should_index(
            visibility,
            &filter_type,
            DocLineCount::new(doc.lines().count()),
        )
    }

    fn process_declaration(
        &self,
        node: tree_sitter::Node,
        item_type: JavaItemType,
        ctx: &ProcessingContext,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let Some(doc) = extract_doc_comment(node, ctx.source_bytes) else {
            return Ok(Vec::new());
        };
        let Some(name) = extract_identifier(node, ctx.source_bytes) else {
            return Ok(Vec::new());
        };
        let (java_vis, vis) = get_visibility(node, ctx.source_bytes);
        if !self.should_index(&vis, item_type, &doc) {
            return Ok(Vec::new());
        }
        let signature = node
            .utf8_text(ctx.source_bytes)
            .ok()
            .and_then(extract_signature)
            .and_then(|s| Signature::try_new(&s).ok());
        let metadata = ChunkMetadata {
            item_name: ItemName::try_new(&name)
                .map_err(crate::knowledge::error::box_err)
                .context(ParseSnafu {
                    file_path: ctx.input.source.file_path.to_string(),
                })?,
            visibility: java_vis,
            signature,
            item_type,
        };
        self.create_chunks(&doc, metadata, ctx.package_name, ctx.input)
    }

    fn create_chunks(
        &self,
        text: &str,
        metadata: ChunkMetadata,
        package_name: &Option<PackageName>,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let chunks: Vec<_> = self.splitter.chunks(text).collect();
        let mut result = Vec::new();

        for chunk_text in chunks {
            let encoding = self
                .tokenizer
                .encode(chunk_text, false)
                .context(ParseSnafu {
                    file_path: input.source.file_path.to_string(),
                })?;

            let token_count = encoding.len().max(1);
            let token_count = TokenCount::try_new(token_count)
                .map_err(crate::knowledge::error::box_err)
                .context(ParseSnafu {
                    file_path: input.source.file_path.to_string(),
                })?;

            let chunk_hash = ChunkHash::from_text(chunk_text);
            let chunk = Chunk::builder()
                .id(crate::knowledge::domain::ChunkId::new(uuid::Uuid::new_v4()))
                .chunk_hash(chunk_hash)
                .file_hash(input.file_hash)
                .source(input.source.clone())
                .content(
                    ChunkContent::builder()
                        .text(chunk_text.to_string())
                        .token_count(token_count)
                        .build(),
                )
                .context(ChunkContext::JavaDoc(
                    JavaDocContext::builder()
                        .item_name(metadata.item_name.clone())
                        .visibility(metadata.visibility)
                        .maybe_signature(metadata.signature.clone())
                        .item_type(metadata.item_type)
                        .maybe_package_name(package_name.as_ref().cloned())
                        .build(),
                ))
                .build();

            result.push(chunk);
        }

        Ok(result)
    }
}

fn extract_signature(text: &str) -> Option<String> {
    text.lines().next().map(|s| s.trim().to_string())
}

fn get_item_type(kind: &str) -> Option<JavaItemType> {
    match kind {
        node_kinds::CLASS_DECLARATION => Some(JavaItemType::Class),
        node_kinds::INTERFACE_DECLARATION => Some(JavaItemType::Interface),
        node_kinds::ENUM_DECLARATION => Some(JavaItemType::Enum),
        node_kinds::RECORD_DECLARATION => Some(JavaItemType::Record),
        node_kinds::METHOD_DECLARATION => Some(JavaItemType::Method),
        node_kinds::CONSTRUCTOR_DECLARATION => Some(JavaItemType::Constructor),
        node_kinds::FIELD_DECLARATION => Some(JavaItemType::Field),
        node_kinds::ANNOTATION_TYPE_DECLARATION => Some(JavaItemType::Annotation),
        _ => None,
    }
}

impl ChunkingStrategy for JavaDocChunker {
    fn supports(&self, file_path: &Path) -> bool {
        file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "java")
            .unwrap_or(false)
    }

    fn chunk(&self, input: &ChunkingInput) -> Result<Vec<Chunk>, ChunkingError> {
        use super::strategy::chunking_error::*;
        use snafu::ResultExt;

        let content = input.content.as_ref();
        let file_path = input.source.file_path.to_string();

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .map_err(crate::knowledge::error::box_err)
            .context(ParseSnafu {
                file_path: file_path.clone(),
            })?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| ChunkingError::ParseError {
                file_path: file_path.clone(),
                source: "Failed to parse Java source".into(),
            })?;

        let root = tree.root_node();
        let source_bytes = content.as_bytes();
        let mut chunks = Vec::new();
        let package_name = extract_package_name(root, source_bytes);
        let ctx = ProcessingContext {
            source_bytes,
            package_name: &package_name,
            input,
        };

        process_node(self, root, &ctx, &mut chunks)?;

        Ok(chunks)
    }
}

fn extract_package_name(root: tree_sitter::Node, source: &[u8]) -> Option<PackageName> {
    root.children(&mut root.walk())
        .find(|c| c.kind() == "package_declaration")
        .and_then(|pkg| {
            pkg.children(&mut pkg.walk())
                .find(|c| c.kind() == "scoped_identifier" || c.kind() == "identifier")
        })
        .and_then(|n| n.utf8_text(source).ok())
        .and_then(|s| PackageName::try_new(s).ok())
}

fn process_node(
    chunker: &JavaDocChunker,
    node: tree_sitter::Node,
    ctx: &ProcessingContext,
    chunks: &mut Vec<Chunk>,
) -> Result<(), ChunkingError> {
    if let Some(item_type) = get_item_type(node.kind()) {
        chunks.extend(chunker.process_declaration(node, item_type, ctx)?);
    }
    for child in node.children(&mut node.walk()) {
        process_node(chunker, child, ctx, chunks)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::chunking::ChunkingStrategy;
    use crate::knowledge::domain::{
        ChunkContext, ChunkSource, ChunkableContent, FileHash, IndexRelativePath, RepoName,
    };

    fn test_config() -> EmbeddingModelConfig {
        EmbeddingModelConfig::default()
    }

    fn make_input(content: &str) -> ChunkingInput {
        ChunkingInput {
            content: ChunkableContent::new(content.to_string()),
            source: ChunkSource::builder()
                .file_path(IndexRelativePath::try_new("Test.java").unwrap())
                .repo_name(RepoName::try_new("test").unwrap())
                .build(),
            file_hash: FileHash::new([0u8; 32]),
        }
    }

    #[test]
    fn test_supports_java_files() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        assert!(chunker.supports(Path::new("Main.java")));
        assert!(chunker.supports(Path::new("pkg/Util.java")));
        assert!(!chunker.supports(Path::new("main.rs")));
        assert!(!chunker.supports(Path::new("Main.java.bak")));
    }

    #[test]
    fn test_extracts_class_doc() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        let input = make_input(
            r#"
package com.example;

/**
 * A sample class.
 * With multiple lines.
 */
public class Sample {
}
"#,
        );
        let chunks = chunker.chunk(&input).unwrap();
        assert!(!chunks.is_empty());
        let ctx = match &chunks[0].context {
            ChunkContext::JavaDoc(ctx) => ctx,
            _ => panic!("Expected JavaDoc context"),
        };
        assert_eq!(ctx.item_name.to_string().as_str(), "Sample");
        assert_eq!(ctx.visibility, JavaVisibility::Public);
        assert_eq!(ctx.item_type, JavaItemType::Class);
    }

    #[test]
    fn test_extracts_method_doc() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        let input = make_input(
            r#"
package com.example;

public class Sample {
    /**
     * Does something useful.
     */
    public void doSomething() {}
}
"#,
        );
        let chunks = chunker.chunk(&input).unwrap();
        let method_chunk = chunks.iter().find(|c| {
            matches!(&c.context, ChunkContext::JavaDoc(ctx) if ctx.item_type == JavaItemType::Method)
        });
        assert!(method_chunk.is_some());
        let ctx = match &method_chunk.unwrap().context {
            ChunkContext::JavaDoc(ctx) => ctx,
            _ => panic!("Expected JavaDoc context"),
        };
        assert_eq!(ctx.item_name.to_string().as_str(), "doSomething");
    }

    #[test]
    fn test_skips_undocumented() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        let input = make_input(
            r#"
package com.example;

public class NoDoc {
}
"#,
        );
        let chunks = chunker.chunk(&input).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_extracts_package_name() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        let input = make_input(
            r#"
package com.example.mypackage;

/**
 * A class.
 */
public class Foo {}
"#,
        );
        let chunks = chunker.chunk(&input).unwrap();
        assert!(!chunks.is_empty());
        let ctx = match &chunks[0].context {
            ChunkContext::JavaDoc(ctx) => ctx,
            _ => panic!("Expected JavaDoc context"),
        };
        assert_eq!(
            ctx.package_name.as_ref().map(|p| p.to_string()).as_deref(),
            Some("com.example.mypackage")
        );
    }

    #[test]
    fn test_package_private_visibility() {
        let chunker = JavaDocChunker::from_config(&test_config()).unwrap();
        let input = make_input(
            r#"
package com.example;

/**
 * Package private class.
 */
class Internal {
}
"#,
        );
        let chunks = chunker.chunk(&input).unwrap();
        assert!(!chunks.is_empty());
        let ctx = match &chunks[0].context {
            ChunkContext::JavaDoc(ctx) => ctx,
            _ => panic!("Expected JavaDoc context"),
        };
        assert_eq!(ctx.visibility, JavaVisibility::PackagePrivate);
    }
}
