//! Markdown chunking strategy

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::path::Path;

use super::{ChunkingError, ChunkingInput, ChunkingStrategy};
use crate::knowledge::domain::{
    Chunk, ChunkContent, ChunkContext, ChunkId, HeadingText, MarkdownContext, TokenCount,
};

/// Chunks markdown files by heading structure
pub struct MarkdownChunker;

impl MarkdownChunker {
    fn estimate_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    fn heading_level_to_depth(level: HeadingLevel) -> usize {
        match level {
            HeadingLevel::H1 => 0,
            HeadingLevel::H2 => 1,
            HeadingLevel::H3 => 2,
            HeadingLevel::H4 => 3,
            HeadingLevel::H5 => 4,
            HeadingLevel::H6 => 5,
        }
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
        let parser = Parser::new(&content);

        let mut chunks = Vec::new();
        let mut current_text = String::new();
        let mut heading_stack: Vec<String> = Vec::new();
        let mut in_heading = false;
        let mut current_heading_text = String::new();
        let mut current_heading_level = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    if !current_text.trim().is_empty() {
                        let token_count = Self::estimate_tokens(&current_text);

                        let chunk = Chunk::builder()
                            .id(ChunkId::new(uuid::Uuid::new_v4()))
                            .source(input.source.clone())
                            .content(
                                ChunkContent::builder()
                                    .text(current_text.trim())
                                    .token_count(TokenCount::try_new(token_count.max(1)).unwrap())
                                    .build(),
                            )
                            .context(ChunkContext::Markdown(
                                MarkdownContext::builder()
                                    .heading_hierarchy(
                                        heading_stack
                                            .iter()
                                            .filter_map(|h| HeadingText::try_new(h.clone()).ok())
                                            .collect::<Vec<_>>(),
                                    )
                                    .build(),
                            ))
                            .indexed_at(std::time::SystemTime::now())
                            .index_data(crate::knowledge::domain::IndexData::Fast {
                                bm25_terms: std::collections::BTreeMap::new(),
                            })
                            .build();

                        chunks.push(chunk);
                        current_text.clear();
                    }

                    in_heading = true;
                    current_heading_level = Self::heading_level_to_depth(level);
                    current_heading_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;

                    heading_stack.truncate(current_heading_level);
                    heading_stack.push(current_heading_text.clone());
                }
                Event::Text(text) => {
                    if in_heading {
                        current_heading_text.push_str(&text);
                    } else {
                        current_text.push_str(&text);
                    }
                }
                Event::Code(text) | Event::Html(text) => {
                    if !in_heading {
                        current_text.push_str(&text);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !in_heading {
                        current_text.push('\n');
                    }
                }
                _ => {}
            }
        }

        if !current_text.trim().is_empty() {
            let token_count = Self::estimate_tokens(&current_text);

            let chunk = Chunk::builder()
                .id(ChunkId::new(uuid::Uuid::new_v4()))
                .source(input.source.clone())
                .content(
                    ChunkContent::builder()
                        .text(current_text.trim())
                        .token_count(TokenCount::try_new(token_count.max(1)).unwrap())
                        .build(),
                )
                .context(ChunkContext::Markdown(
                    MarkdownContext::builder()
                        .heading_hierarchy(
                            heading_stack
                                .iter()
                                .filter_map(|h| HeadingText::try_new(h.clone()).ok())
                                .collect::<Vec<_>>(),
                        )
                        .build(),
                ))
                .indexed_at(std::time::SystemTime::now())
                .index_data(crate::knowledge::domain::IndexData::Fast {
                    bm25_terms: std::collections::BTreeMap::new(),
                })
                .build();

            chunks.push(chunk);
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

    fn create_test_input(content: &str) -> ChunkingInput {
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
            .max_tokens(TokenCount::try_new(500).unwrap())
            .build()
    }

    #[test]
    fn test_supports_markdown_files() {
        // Given A markdown chunker
        let chunker = MarkdownChunker;

        // When Checking if it supports .md files
        let supports = chunker.supports(Path::new("test.md"));

        // Then It should return true
        assert!(supports);
    }

    #[test]
    fn test_does_not_support_non_markdown() {
        // Given A markdown chunker
        let chunker = MarkdownChunker;

        // When Checking if it supports .rs files
        let supports = chunker.supports(Path::new("test.rs"));

        // Then It should return false
        assert!(!supports);
    }

    #[test]
    fn test_chunks_by_h2_headings() {
        // Given Markdown with multiple H2 sections
        let content = r#"# Title

Some intro text.

## Section One

Content for section one.

## Section Two

Content for section two."#;

        let input = create_test_input(content);
        let chunker = MarkdownChunker;

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create chunks for each section
        assert_eq!(chunks.len(), 3);

        // And First chunk should have intro text
        assert!(chunks[0].content.text.contains("Some intro text"));

        // And Second chunk should have section one content
        assert!(chunks[1].content.text.contains("Content for section one"));

        // And Third chunk should have section two content
        assert!(chunks[2].content.text.contains("Content for section two"));
    }

    #[test]
    fn test_builds_heading_hierarchy() {
        // Given Markdown with nested headings
        let content = r#"## Parent

Parent content.

### Child

Child content."#;

        let input = create_test_input(content);
        let chunker = MarkdownChunker;

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should have two chunks
        assert_eq!(chunks.len(), 2);

        // And First chunk should have parent in hierarchy
        if let ChunkContext::Markdown(ctx) = &chunks[0].context {
            assert_eq!(ctx.heading_hierarchy.len(), 1);
            assert_eq!(
                ctx.heading_hierarchy[0],
                HeadingText::try_new("Parent").unwrap()
            );
        } else {
            panic!("Expected Markdown context");
        }

        // And Second chunk should have both parent and child
        if let ChunkContext::Markdown(ctx) = &chunks[1].context {
            assert_eq!(ctx.heading_hierarchy.len(), 2);
            assert_eq!(
                ctx.heading_hierarchy[0],
                HeadingText::try_new("Parent").unwrap()
            );
            assert_eq!(
                ctx.heading_hierarchy[1],
                HeadingText::try_new("Child").unwrap()
            );
        } else {
            panic!("Expected Markdown context");
        }
    }

    #[test_case("# H1\nContent", 1 ; "single h1")]
    #[test_case("## H2\nContent", 1 ; "single h2")]
    #[test_case("### H3\nContent", 1 ; "single h3")]
    fn test_different_heading_levels(content: &str, expected_chunks: usize) {
        // Given Markdown with specific heading level
        let input = create_test_input(content);
        let chunker = MarkdownChunker;

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create expected number of chunks
        assert_eq!(chunks.len(), expected_chunks);
    }

    #[test]
    fn test_empty_content() {
        // Given Empty markdown content
        let input = create_test_input("");
        let chunker = MarkdownChunker;

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should return no chunks
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_content_without_headings() {
        // Given Markdown without any headings
        let content = "Just some plain text without headings.";
        let input = create_test_input(content);
        let chunker = MarkdownChunker;

        // When Chunking the content
        let chunks = chunker.chunk(&input).unwrap();

        // Then Should create one chunk
        assert_eq!(chunks.len(), 1);

        // And Chunk should have empty heading hierarchy
        if let ChunkContext::Markdown(ctx) = &chunks[0].context {
            assert_eq!(ctx.heading_hierarchy.len(), 0);
        } else {
            panic!("Expected Markdown context");
        }
    }
}
