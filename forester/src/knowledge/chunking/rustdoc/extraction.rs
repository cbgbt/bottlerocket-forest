//! Doc comment extraction logic for Rust items.

use syn::{Attribute, Item, ItemImpl};
use text_splitter::TextSplitter;
use tokenizers::Tokenizer;

use crate::knowledge::chunking::{ChunkingError, ChunkingInput};
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, ItemName, RustDocContext, Signature, TokenCount,
    Visibility,
};

pub(crate) struct DocExtractor<'a> {
    pub(super) splitter: &'a TextSplitter<Tokenizer>,
    pub(super) tokenizer: &'a Tokenizer,
    pub(super) filter: Option<&'a crate::knowledge::indexing::RustFilter>,
}

impl<'a> DocExtractor<'a> {
    /// Extracts doc comment text from attributes.
    ///
    /// Collects `///` and `//!` comments, ignoring inline `//` comments.
    pub(super) fn extract_doc_text(attrs: &[Attribute]) -> String {
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
    pub(super) fn extract_item_doc_chunks(
        &self,
        item: &Item,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use crate::knowledge::chunking::strategy::chunking_error::*;
        use crate::knowledge::indexing::RustItemType;
        use snafu::ResultExt;

        let file_path = input.source.file_path.to_string();
        let mut chunks = Vec::new();

        match item {
            Item::Fn(item_fn) => {
                let doc_text = Self::extract_doc_text(&item_fn.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_fn.sig.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    let sig = &item_fn.sig;
                    let signature = Signature::try_new(quote::quote!(#sig).to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_fn.vis.clone()),
                        Some(signature),
                        RustItemType::Function,
                        input,
                    )?);
                }
            }
            Item::Struct(item_struct) => {
                let doc_text = Self::extract_doc_text(&item_struct.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_struct.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_struct.vis.clone()),
                        None,
                        RustItemType::Struct,
                        input,
                    )?);
                }
            }
            Item::Enum(item_enum) => {
                let doc_text = Self::extract_doc_text(&item_enum.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_enum.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_enum.vis.clone()),
                        None,
                        RustItemType::Enum,
                        input,
                    )?);
                }
            }
            Item::Trait(item_trait) => {
                let doc_text = Self::extract_doc_text(&item_trait.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_trait.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_trait.vis.clone()),
                        None,
                        RustItemType::Trait,
                        input,
                    )?);
                }
            }
            Item::Mod(item_mod) => {
                let doc_text = Self::extract_doc_text(&item_mod.attrs);
                let mod_name = ItemName::try_new(item_mod.ident.to_string())
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(ParseSnafu {
                        file_path: file_path.clone(),
                    })?;

                if !doc_text.trim().is_empty() {
                    chunks.extend(self.create_chunks(
                        &doc_text,
                        mod_name,
                        Visibility::from(item_mod.vis.clone()),
                        None,
                        RustItemType::Module,
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
            Item::Type(item_type) => {
                let doc_text = Self::extract_doc_text(&item_type.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_type.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_type.vis.clone()),
                        None,
                        RustItemType::TypeAlias,
                        input,
                    )?);
                }
            }
            Item::Const(item_const) => {
                let doc_text = Self::extract_doc_text(&item_const.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(item_const.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(item_const.vis.clone()),
                        None,
                        RustItemType::Constant,
                        input,
                    )?);
                }
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
        use crate::knowledge::chunking::strategy::chunking_error::*;
        use crate::knowledge::indexing::RustItemType;
        use snafu::ResultExt;

        let file_path = input.source.file_path.to_string();
        let mut chunks = Vec::new();

        for impl_item in &item_impl.items {
            if let syn::ImplItem::Fn(method) = impl_item {
                let doc_text = Self::extract_doc_text(&method.attrs);
                if !doc_text.trim().is_empty() {
                    let item_name = ItemName::try_new(method.sig.ident.to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    let sig = &method.sig;
                    let signature = Signature::try_new(quote::quote!(#sig).to_string())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        .context(ParseSnafu {
                            file_path: file_path.clone(),
                        })?;

                    chunks.extend(self.create_chunks(
                        &doc_text,
                        item_name,
                        Visibility::from(method.vis.clone()),
                        Some(signature),
                        RustItemType::Impl,
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
    pub(super) fn create_chunks(
        &self,
        doc_text: &str,
        item_name: ItemName,
        visibility: Visibility,
        signature: Option<Signature>,
        item_type: crate::knowledge::indexing::RustItemType,
        input: &ChunkingInput,
    ) -> Result<Vec<Chunk>, ChunkingError> {
        use crate::knowledge::chunking::strategy::chunking_error::*;
        use snafu::ResultExt;

        if let Some(filter) = &self.filter
            && !filter.should_index(&visibility, &item_type, doc_text.lines().count())
        {
            return Ok(vec![]);
        }

        let file_path = input.source.file_path.to_string();
        let text_chunks: Vec<&str> = self.splitter.chunks(doc_text).collect();

        text_chunks
            .into_iter()
            .map(|text| {
                let encoding = self.tokenizer.encode(text, false).context(ParseSnafu {
                    file_path: file_path.clone(),
                })?;

                let token_count = encoding.len().max(1);

                let token_count = TokenCount::try_new(token_count)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(ParseSnafu {
                        file_path: file_path.clone(),
                    })?;

                let context = RustDocContext::builder()
                    .item_name(item_name.clone())
                    .visibility(visibility)
                    .maybe_signature(signature.clone())
                    .item_type(item_type)
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
                    .build())
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::chunking::{ChunkingStrategy, RustDocChunker};
    use crate::knowledge::domain::EmbeddingModelConfig;
    use crate::knowledge::domain::{
        ChunkContext, ChunkSource, ChunkableContent, ForestRelativePath, ItemName, RepoName,
        Visibility,
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
        }
    }

    #[test_case(r#"
/// Validates input data
pub fn validate(input: &str) -> bool {
    true
}
"#, "validate" ; "fn_item")]
    #[test_case(r#"
/// User profile data
pub struct User {
    name: String,
}
"#, "User" ; "struct_item")]
    #[test_case(r#"
/// Connection state
pub enum State {
    Connected,
    Disconnected,
}
"#, "State" ; "enum_item")]
    #[test_case(r#"
/// Repository for data persistence
pub trait Repository {
    fn save(&self);
}
"#, "Repository" ; "trait_item")]
    fn test_captures_item_metadata(content: &str, expected_name: &str) {
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.item_name, ItemName::try_new(expected_name).unwrap());
            assert_eq!(ctx.visibility, Visibility::Public);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test_case("pub", Visibility::Public ; "public_vis")]
    #[test_case("pub(crate)", Visibility::Crate ; "crate_vis")]
    #[test_case("", Visibility::Private ; "private_vis")]
    fn test_captures_visibility(vis_modifier: &str, expected_visibility: Visibility) {
        let content = format!(
            r#"
/// Documented function
{} fn test_fn() {{}}
"#,
            vis_modifier
        );
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        assert!(!chunks.is_empty());
        if let ChunkContext::RustDoc(ctx) = &chunks[0].context {
            assert_eq!(ctx.visibility, expected_visibility);
        } else {
            panic!("Expected RustDoc context");
        }
    }

    #[test]
    fn test_captures_function_signature() {
        let content = r#"
/// Processes data
pub fn process(input: &str, count: usize) -> Result<String, std::io::Error> {
    Ok(input.to_string())
}
"#;
        let input = create_test_input(content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

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
        let long_doc = format!("/// {}\n", "word ".repeat(300));
        let content = format!("{}pub fn test() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

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
        let long_doc = format!("/// {}\n", "This is sentence number X. ".repeat(200));
        let content = format!("{}pub fn test() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

        if chunks.len() > 1 {
            let first_chunk_end = &chunks[0].content.text[chunks[0].content.text.len() - 50..];
            let second_chunk_start =
                &chunks[1].content.text[..50.min(chunks[1].content.text.len())];

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
        let long_doc = format!("/// {}\n", "word ".repeat(300));
        let content = format!("{}pub fn long_function() {{}}", long_doc);
        let input = create_test_input(&content);
        let config = test_config();
        let chunker = RustDocChunker::from_config(&config).unwrap();

        let chunks = chunker.chunk(&input).unwrap();

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
    fn test_chunker_respects_visibility_filter() {
        use crate::knowledge::domain::Visibility;
        use crate::knowledge::indexing::{RustFilter, RustItemType};

        let config = test_config();
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Function], 0);
        let chunker = RustDocChunker::from_config_with_filter(&config, Some(filter)).unwrap();

        let input = create_test_input(
            r#"
/// Public function
pub fn public_fn() {}

/// Private function
fn private_fn() {}
"#,
        );

        let result = chunker.chunk(&input);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.text.contains("Public function"));
    }

    #[test]
    fn test_chunker_respects_item_type_filter() {
        use crate::knowledge::domain::Visibility;
        use crate::knowledge::indexing::{RustFilter, RustItemType};

        let config = test_config();
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Struct], 0);
        let chunker = RustDocChunker::from_config_with_filter(&config, Some(filter)).unwrap();

        let input = create_test_input(
            r#"
/// A struct
pub struct MyStruct {}

/// A function
pub fn my_function() {}
"#,
        );

        let result = chunker.chunk(&input);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.text.contains("A struct"));
    }

    #[test]
    fn test_chunker_respects_min_doc_lines_filter() {
        use crate::knowledge::domain::Visibility;
        use crate::knowledge::indexing::{RustFilter, RustItemType};

        let config = test_config();
        let filter = RustFilter::new(vec![Visibility::Public], vec![RustItemType::Function], 3);
        let chunker = RustDocChunker::from_config_with_filter(&config, Some(filter)).unwrap();

        let input = create_test_input(
            r#"
/// Short doc
pub fn short() {}

/// This is a longer documentation comment
/// that spans multiple lines
/// and exceeds the minimum line count
pub fn long() {}
"#,
        );

        let result = chunker.chunk(&input);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.text.contains("longer documentation"));
    }

    #[test]
    fn test_chunker_filters_multiple_criteria() {
        use crate::knowledge::domain::Visibility;
        use crate::knowledge::indexing::{RustFilter, RustItemType};

        let config = test_config();
        let filter = RustFilter::new(
            vec![Visibility::Public],
            vec![RustItemType::Struct, RustItemType::Enum],
            2,
        );
        let chunker = RustDocChunker::from_config_with_filter(&config, Some(filter)).unwrap();

        let input = create_test_input(
            r#"
/// A public struct with
/// sufficient documentation
pub struct PublicStruct {}

/// Short
pub struct ShortDoc {}

/// A private struct with
/// sufficient documentation
struct PrivateStruct {}

/// A public enum with
/// sufficient documentation
pub enum PublicEnum { A, B }

/// A public function with
/// sufficient documentation
pub fn public_function() {}
"#,
        );

        let result = chunker.chunk(&input);

        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
        let texts: Vec<&str> = chunks.iter().map(|c| c.content.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("public struct")));
        assert!(texts.iter().any(|t| t.contains("public enum")));
    }
}
