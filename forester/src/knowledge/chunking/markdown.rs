//! Markdown chunking strategy

use snafu::ResultExt;
use std::path::Path;
use text_splitter::{ChunkConfig, MarkdownSplitter};
use tokenizers::Tokenizer;

use super::{ChunkingConfig, ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, HeadingText, MarkdownContext, TokenCount,
};

/// Chunks markdown files by heading structure
pub struct MarkdownChunker {
    splitter: MarkdownSplitter<Tokenizer>,
}

impl MarkdownChunker {
    pub fn new(config: &ChunkingConfig) -> Result<Self, ChunkingError> {
        use super::strategy::chunking_error::*;

        let tokenizer = Tokenizer::from_pretrained(&config.tokenizer_model, None)
            .map_err(|e| e as Box<dyn std::error::Error + Send + Sync>)
            .context(ParseSnafu)?;

        let splitter = MarkdownSplitter::new(
            ChunkConfig::new(config.max_tokens)
                .with_sizer(tokenizer)
                .with_overlap(config.overlap_tokens)
                .expect("overlap configuration should be valid"),
        );

        Ok(Self { splitter })
    }

    fn split_by_headings(&self, content: &str) -> Vec<(String, Vec<HeadingText>)> {
        let mut sections = Vec::new();
        let mut current_section = String::new();
        let mut current_hierarchy: Vec<HeadingText> = Vec::new();
        let mut level_stack: Vec<(usize, String)> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                if !current_section.trim().is_empty() {
                    sections.push((current_section.clone(), current_hierarchy.clone()));
                    current_section.clear();
                }

                let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
                let text = trimmed[hash_count..].trim();

                if !text.is_empty() {
                    while let Some((last_level, _)) = level_stack.last() {
                        if *last_level >= hash_count {
                            level_stack.pop();
                        } else {
                            break;
                        }
                    }

                    level_stack.push((hash_count, text.to_string()));

                    current_hierarchy = level_stack
                        .iter()
                        .filter_map(|(_, t)| HeadingText::try_new(t.clone()).ok())
                        .collect();
                }

                current_section.push_str(line);
                current_section.push('\n');
            } else {
                current_section.push_str(line);
                current_section.push('\n');
            }
        }

        if !current_section.trim().is_empty() {
            sections.push((current_section, current_hierarchy));
        }

        sections
    }
}

impl ChunkingStrategy for MarkdownChunker {
    fn supports(&self, file_path: &Path) -> bool {
        file_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext == "md")
            .unwrap_or(false)
    }

    fn chunk(&self, input: &ChunkingInput) -> Result<Vec<Chunk>, ChunkingError> {
        let content = input.content.clone().into_inner();

        let sections = self.split_by_headings(&content);
        let mut chunks = Vec::new();

        for (section_text, hierarchy) in sections {
            let text_chunks = self.splitter.chunks(&section_text);

            for chunk_text in text_chunks {
                let trimmed = chunk_text.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let has_body_content = trimmed
                    .lines()
                    .any(|line| !line.trim().is_empty() && !line.trim().starts_with('#'));

                if !has_body_content {
                    continue;
                }

                let token_count = trimmed.split_whitespace().count().max(1);

                let chunk = Chunk::builder()
                    .id(ChunkId::new(uuid::Uuid::new_v4()))
                    .source(input.source.clone())
                    .content(
                        ChunkContent::builder()
                            .text(trimmed)
                            .token_count(TokenCount::try_new(token_count).unwrap())
                            .build(),
                    )
                    .context(ChunkContext::Markdown(
                        MarkdownContext::builder()
                            .heading_hierarchy(hierarchy.clone())
                            .build(),
                    ))
                    .indexed_at(std::time::SystemTime::now())
                    .index_data(crate::knowledge::domain::IndexData::Fast {
                        bm25_terms: std::collections::BTreeMap::new(),
                    })
                    .build();

                chunks.push(chunk);
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        ChunkSource, ChunkableContent, ForestRelativePath, LineCount, LineNumber, LineRange,
        RepoName,
    };
    use test_case::test_case;

    fn test_config() -> ChunkingConfig {
        ChunkingConfig::builder().build()
    }

    fn create_test_input(content: &str, max_tokens: usize) -> ChunkingInput {
        ChunkingInput::builder()
            .content(ChunkableContent::new(content.to_string()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .line_range(
                        LineRange::builder()
                            .start(LineNumber::try_new(1).unwrap())
                            .line_count(LineCount::try_new(10).unwrap())
                            .build(),
                    )
                    .build(),
            )
            .max_tokens(TokenCount::try_new(max_tokens).unwrap())
            .build()
    }

    #[test]
    fn test_supports_markdown_files() {
        // Given A markdown chunker
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Checking if it supports .md files
        let supports = chunker.supports(Path::new("test.md"));

        // Then It should return true
        assert!(supports);
    }

    #[test_case("test.md" => true ; "markdown extension")]
    #[test_case("test.rs" => false ; "rust extension")]
    #[test_case("test.txt" => false ; "text extension")]
    #[test_case("test" => false ; "no extension")]
    fn test_file_extension_support(path: &str) -> bool {
        // Given A markdown chunker
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Checking file support
        // Then Return whether it's supported
        chunker.supports(Path::new(path))
    }

    #[test]
    fn test_respects_token_limit() {
        // Given Markdown with content that exceeds token limit
        let content = format!(
            "# Long Section\n\n{}",
            "word ".repeat(300) // 300 words should exceed 256 tokens
        );
        let input = create_test_input(&content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

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
        // Given Markdown with long content under one heading
        let content = format!("# Section\n\n{}", "This is sentence number X. ".repeat(200));
        let input = create_test_input(&content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

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
    fn test_preserves_heading_hierarchy_across_chunks() {
        // Given Markdown with nested headings and long content
        let content = format!(
            "# Top\n\n## Middle\n\n{}",
            "word ".repeat(300) // Force multiple chunks
        );
        let input = create_test_input(&content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then All chunks under same heading should have same hierarchy
        for chunk in &chunks {
            if let ChunkContext::Markdown(ctx) = &chunk.context {
                assert!(
                    !ctx.heading_hierarchy.is_empty(),
                    "Chunks should preserve heading hierarchy"
                );
            }
        }
    }

    #[test]
    fn test_empty_content() {
        // Given Empty markdown content
        let input = create_test_input("", 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should return no chunks
        assert_eq!(chunks.len(), 0);
    }

    #[test_case("" => 0 ; "empty string")]
    #[test_case("   \n\n  " => 0 ; "only whitespace")]
    #[test_case("# Heading Only" => 0 ; "heading with no content")]
    fn test_no_chunks_for_empty_sections(content: &str) -> usize {
        // Given Content with no substantive text
        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Return chunk count
        chunks.len()
    }

    #[test]
    fn test_content_without_headings() {
        // Given Markdown without any headings
        let content = "Just some plain text without headings.";
        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create at least one chunk
        assert!(!chunks.is_empty());

        // And Chunk should have empty heading hierarchy
        if let ChunkContext::Markdown(ctx) = &chunks[0].context {
            assert_eq!(ctx.heading_hierarchy.len(), 0);
        } else {
            panic!("Expected Markdown context");
        }
    }

    #[test]
    fn test_multiple_sections_create_separate_chunks() {
        // Given Markdown with multiple distinct sections
        let content = r#"# First

First section content.

# Second

Second section content.

# Third

Third section content."#;

        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create separate chunks for each section
        assert!(chunks.len() >= 3, "Expected at least 3 chunks");

        // And Each chunk should have different heading
        let headings: Vec<_> = chunks
            .iter()
            .filter_map(|c| {
                if let ChunkContext::Markdown(ctx) = &c.context {
                    ctx.heading_hierarchy.first().cloned()
                } else {
                    None
                }
            })
            .collect();

        assert!(headings.contains(&HeadingText::try_new("First").unwrap()));
        assert!(headings.contains(&HeadingText::try_new("Second").unwrap()));
        assert!(headings.contains(&HeadingText::try_new("Third").unwrap()));
    }

    #[test]
    fn test_nested_headings_build_hierarchy() {
        // Given Markdown with deeply nested headings
        let content = r#"# Level 1

## Level 2

### Level 3

Content at level 3."#;

        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Last chunk should have full hierarchy
        let last_chunk = chunks.last().expect("Should have at least one chunk");
        if let ChunkContext::Markdown(ctx) = &last_chunk.context {
            assert_eq!(ctx.heading_hierarchy.len(), 3);
            assert_eq!(
                ctx.heading_hierarchy[0],
                HeadingText::try_new("Level 1").unwrap()
            );
            assert_eq!(
                ctx.heading_hierarchy[1],
                HeadingText::try_new("Level 2").unwrap()
            );
            assert_eq!(
                ctx.heading_hierarchy[2],
                HeadingText::try_new("Level 3").unwrap()
            );
        } else {
            panic!("Expected Markdown context");
        }
    }

    #[test]
    fn test_code_blocks_included_in_chunks() {
        // Given Markdown with code blocks
        let content = r#"# Example

Here is some code:

```rust
fn main() {
    println!("Hello");
}
```

More text after code."#;

        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Code should be included in chunk text
        assert!(!chunks.is_empty());
        let combined_text = chunks
            .iter()
            .map(|c| c.content.text.as_str())
            .collect::<String>();
        assert!(combined_text.contains("fn main"));
        assert!(combined_text.contains("println"));
    }

    #[test]
    fn test_all_chunks_have_valid_token_counts() {
        // Given Various markdown content
        let content = r#"# Section

Some content here.

## Subsection

More content in subsection."#;

        let input = create_test_input(content, 256);
        let config = test_config();
        let chunker = MarkdownChunker::new(&config).unwrap();

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then All chunks should have token count greater than zero
        for chunk in &chunks {
            assert!(
                chunk.content.token_count.into_inner() > 0,
                "Chunk should have positive token count"
            );
        }
    }
}
