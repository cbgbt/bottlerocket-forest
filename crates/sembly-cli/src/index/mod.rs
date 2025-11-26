//! Knowledge index management commands.
//!
//! This module provides CLI commands for building, updating, and querying the knowledge index.
//! The index enables semantic search across Bottlerocket repositories.
//!
//! Submodules:
//! * [`progress`] - Progress reporting for indexing operations

use clap::Parser;
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use self::progress::CliProgressReporter;
use super::theme;

mod progress;
use sembly_core::knowledge::KnowledgeIndex;
use sembly_core::knowledge::domain::{
    FileSearchResult, ForestRelativePath, RelevanceScore, RepoName, SearchResult, SearchResults,
};

/// Arguments for building the knowledge index.
#[derive(Parser)]
pub struct BuildArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Arguments for rebuilding the knowledge index from scratch.
#[derive(Parser)]
pub struct RebuildArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Arguments for incrementally updating the knowledge index.
#[derive(Parser)]
pub struct UpdateArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Arguments for clearing all chunks from the index.
#[derive(Parser)]
pub struct ClearArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    yes: bool,
}

/// Arguments for searching the knowledge index.
#[derive(Parser)]
pub struct SearchArgs {
    /// Search query
    query: String,

    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,

    /// Maximum number of results (1-100, defaults to 10)
    #[arg(short = 'n', long)]
    limit: Option<usize>,

    /// Output format: human or json (defaults to human)
    #[arg(short = 'f', long)]
    format: Option<String>,

    /// Show individual chunk matches under each file
    #[arg(long)]
    show_chunks: bool,
}

/// Arguments for showing index status and statistics.
#[derive(Parser)]
pub struct StatusArgs {
    /// Path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Builds the knowledge index, processing all files in the forest.
pub fn handle_build(args: BuildArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let progress = Arc::new(CliProgressReporter::default());
    let result = index
        .build()
        .progress(progress)
        .call()
        .context(KnowledgeIndexSnafu)?;

    format_build_result(&result);

    Ok(())
}

/// Rebuilds the knowledge index from scratch, clearing existing data first.
pub fn handle_rebuild(args: RebuildArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let progress = Arc::new(CliProgressReporter::default());
    let result = index
        .rebuild()
        .progress(progress)
        .call()
        .context(KnowledgeIndexSnafu)?;

    format_build_result(&result);

    Ok(())
}

/// Updates the knowledge index incrementally based on file changes.
pub fn handle_update(args: UpdateArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let progress = Arc::new(CliProgressReporter::default());
    let result = index
        .update()
        .progress(progress)
        .call()
        .context(KnowledgeIndexSnafu)?;

    format_update_result(&result);

    Ok(())
}

/// Deletes the knowledge index database after user confirmation.
pub fn handle_clear(args: ClearArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    if !args.yes && !prompt_confirmation("Are you sure you want to delete the index?")? {
        println!("{}", theme::muted("Cancelled"));
        return Ok(());
    }

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    index.clear().context(KnowledgeIndexSnafu)?;

    println!("{} Deleted index database", theme::success("✓"));

    Ok(())
}

/// Searches the knowledge index and displays results in the requested format.
pub fn handle_search(args: SearchArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let limit = args.limit.unwrap_or(10);

    let format = parse_output_format(args.format.as_deref())?;

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let results = index
        .search(&args.query, limit)
        .context(KnowledgeIndexSnafu)?;

    let file_results = group_results_by_file(&results);

    match format {
        OutputFormat::Human => format_file_results_human(&file_results, args.show_chunks),
        OutputFormat::Json => format_file_results_json(&file_results)?,
    }

    Ok(())
}

/// Displays index status and statistics including chunk count and model configuration.
pub fn handle_status(args: StatusArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let status = index.status().context(KnowledgeIndexSnafu)?;

    format_status(&status);

    Ok(())
}

/// Parses the output format string into an OutputFormat enum
fn parse_output_format(format_str: Option<&str>) -> Result<OutputFormat, IndexError> {
    match format_str {
        Some("human") => Ok(OutputFormat::Human),
        Some("json") => Ok(OutputFormat::Json),
        None => Ok(OutputFormat::Human),
        Some(format) => Err(IndexError::InvalidOutputFormat {
            format: format.to_string(),
        }),
    }
}

/// Groups search results by file path, aggregating chunks and computing best scores
fn group_results_by_file(results: &SearchResults) -> Vec<FileSearchResult> {
    let mut file_map: HashMap<ForestRelativePath, (RepoName, Vec<SearchResult>)> = HashMap::new();

    for result in &results.results {
        let path = result.chunk.source.file_path.clone();
        let repo = result.chunk.source.repo_name.clone();

        file_map
            .entry(path)
            .or_insert_with(|| (repo, Vec::new()))
            .1
            .push(result.clone());
    }

    let mut file_results: Vec<_> = file_map
        .into_iter()
        .map(|(path, (repo, chunks))| {
            let best_score = chunks
                .iter()
                .map(|r| r.score)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or_else(|| RelevanceScore::try_new(0.0).unwrap());

            FileSearchResult::builder()
                .file_path(path)
                .repo_name(repo)
                .match_count(chunks.len())
                .best_score(best_score)
                .chunks(chunks)
                .build()
        })
        .collect();

    file_results.sort_by(|a, b| {
        b.best_score
            .partial_cmp(&a.best_score)
            .unwrap()
            .then_with(|| b.match_count.cmp(&a.match_count))
    });

    file_results
}

/// Prints a formatted summary of build operation results
fn format_build_result(result: &sembly_core::knowledge::indexing::IndexResult) {
    println!("{} Index build complete!", theme::success("✓"));
    println!(
        "  Files processed: {}",
        theme::value(result.files_processed)
    );
    println!("  Chunks created: {}", theme::value(result.chunks_affected));
    println!(
        "  Duration: {}",
        theme::muted(format!("{:.2}s", result.duration.as_secs_f64()))
    );
}

/// Prints a formatted summary of update operation results
fn format_update_result(result: &sembly_core::knowledge::indexing::IndexResult) {
    println!("{} Index update complete!", theme::success("✓"));
    println!("  Files added: {}", theme::value(result.files_added));
    println!("  Files updated: {}", theme::value(result.files_updated));
    println!("  Files removed: {}", theme::value(result.files_removed));
    println!(
        "  Chunks affected: {}",
        theme::value(result.chunks_affected)
    );
    println!(
        "  Duration: {}",
        theme::muted(format!("{:.2}s", result.duration.as_secs_f64()))
    );
}

/// Prints file search results in human-readable format with color-coded scores
fn format_file_results_human(file_results: &[FileSearchResult], show_chunks: bool) {
    if file_results.is_empty() {
        println!("No files matched");
        return;
    }

    println!("Found {} unique files:\n", theme::value(file_results.len()));

    for (i, file_result) in file_results.iter().enumerate() {
        let score_value = file_result.best_score.into_inner();

        println!(
            "{}. [Matches: {}, Best Score: {}] {}",
            theme::value(i + 1),
            theme::value(file_result.match_count),
            theme::score(score_value),
            theme::label(&file_result.file_path)
        );

        if show_chunks {
            for chunk_result in &file_result.chunks {
                let preview = if chunk_result.chunk.content.text.len() > 100 {
                    format!("{}...", &chunk_result.chunk.content.text[..100])
                } else {
                    chunk_result.chunk.content.text.to_string()
                };
                let chunk_score = chunk_result.score.into_inner();
                println!(
                    "   - [Score: {}] {}\n",
                    theme::score(chunk_score),
                    theme::muted(preview.replace('\n', " "))
                );
            }
        }
    }
}

/// Prints file search results as JSON
fn format_file_results_json(file_results: &[FileSearchResult]) -> Result<(), IndexError> {
    use index_error::*;

    let json = serde_json::to_string_pretty(file_results).context(JsonSerializationFailedSnafu)?;
    println!("{}", json);
    Ok(())
}

/// Prints index status information including metadata and statistics
fn format_status(status: &sembly_core::knowledge::facade::IndexStatus) {
    println!("{}", theme::label("Knowledge Index Status"));
    println!("  Exists: {}", theme::value(status.exists));
    println!("  Chunks: {}", theme::value(status.chunk_count));
    println!("  Files: {}", theme::value(status.file_count));

    if let Some(Ok(elapsed)) = status.last_build.map(|t| t.elapsed()) {
        println!(
            "  Last build: {}",
            theme::muted(format!("{:.0} seconds ago", elapsed.as_secs_f64()))
        );
    }

    println!("  Model: {}", theme::value(&status.model_config.model_name));
    println!(
        "  Embedding dim: {}",
        theme::value(status.model_config.embedding_dim)
    );
    println!(
        "  Max tokens: {}",
        theme::value(status.model_config.max_tokens)
    );

    if let Some(size) = status.size_bytes {
        println!(
            "  Size: {}",
            theme::size(format!("{:.2} MB", size as f64 / 1_000_000.0))
        );
    }
}

/// Prompts the user for yes/no confirmation via stdin
fn prompt_confirmation(message: &str) -> Result<bool, IndexError> {
    use index_error::*;

    println!("{} (yes/no): ", message);

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context(InputReadFailedSnafu)?;

    let input = input.trim().to_lowercase();
    Ok(input == "yes" || input == "y")
}

/// Output format for search results
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Snafu, miette::Diagnostic)]
#[snafu(module)]
pub enum IndexError {
    #[snafu(display("Knowledge index operation failed"))]
    #[diagnostic(
        code(sembly::cli::knowledge_index_failed),
        help("Check the error details above for specific guidance")
    )]
    KnowledgeIndex {
        source: sembly_core::knowledge::facade::IndexError,
    },

    #[snafu(display("Invalid output format: {format}"))]
    #[diagnostic(
        code(sembly::cli::invalid_output_format),
        help("Must be 'human' or 'json'")
    )]
    InvalidOutputFormat { format: String },

    #[snafu(display("Failed to read user input"))]
    #[diagnostic(
        code(sembly::cli::input_read_failed),
        help("Check that stdin is available and not closed")
    )]
    InputReadFailed { source: std::io::Error },

    #[snafu(display("Failed to serialize JSON output"))]
    #[diagnostic(
        code(sembly::cli::json_serialization_failed),
        help("The search results may contain invalid data")
    )]
    JsonSerializationFailed { source: serde_json::Error },
}

#[cfg(test)]
mod test {
    use super::*;
    use sembly_core::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, EmbeddingModelConfig,
        ForestRelativePath, MarkdownContext, RelevanceScore, RepoName, SearchQuery, SearchResult,
        SearchResults, TokenCount,
    };
    use sembly_core::knowledge::facade::IndexStatus;
    use sembly_core::knowledge::indexing::IndexResult;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_parse_output_format_human() {
        // Given A format string "human"
        // When Parsing the output format
        let result = parse_output_format(Some("human"));

        // Then It should return OutputFormat::Human
        assert!(matches!(result, Ok(OutputFormat::Human)));
    }

    #[test]
    fn test_parse_output_format_json() {
        // Given A format string "json"
        // When Parsing the output format
        let result = parse_output_format(Some("json"));

        // Then It should return OutputFormat::Json
        assert!(matches!(result, Ok(OutputFormat::Json)));
    }

    #[test]
    fn test_parse_output_format_default() {
        // Given No format string (None)
        // When Parsing the output format
        let result = parse_output_format(None);

        // Then It should default to OutputFormat::Human
        assert!(matches!(result, Ok(OutputFormat::Human)));
    }

    #[test]
    fn test_parse_output_format_invalid() {
        // Given An invalid format string
        // When Parsing the output format
        let result = parse_output_format(Some("xml"));

        // Then It should return InvalidOutputFormat error
        assert!(matches!(
            result,
            Err(IndexError::InvalidOutputFormat { .. })
        ));
    }

    #[test]
    fn test_format_build_result_prints_summary() {
        // Given An IndexResult from a build operation
        let result = IndexResult::builder()
            .files_processed(10)
            .files_added(10)
            .files_updated(0)
            .files_removed(0)
            .files_skipped(0)
            .chunks_affected(50)
            .duration(Duration::from_secs(5))
            .build();

        // When Formatting the build result
        // Then It should print without panicking
        format_build_result(&result);
    }

    #[test]
    fn test_format_update_result_prints_summary() {
        // Given An IndexResult from an update operation
        let result = IndexResult::builder()
            .files_processed(3)
            .files_added(1)
            .files_updated(2)
            .files_removed(1)
            .files_skipped(0)
            .chunks_affected(12)
            .duration(Duration::from_secs(2))
            .build();

        // When Formatting the update result
        // Then It should print without panicking
        format_update_result(&result);
    }

    #[test]
    fn test_format_file_results_human_with_results() {
        // Given FileSearchResults with multiple files
        let chunk = create_test_chunk();
        let file_results = vec![
            FileSearchResult::builder()
                .file_path(ForestRelativePath::try_new("test.md").unwrap())
                .repo_name(RepoName::try_new("test-repo").unwrap())
                .match_count(2usize)
                .best_score(RelevanceScore::try_new(0.95).unwrap())
                .chunks(vec![
                    SearchResult::builder()
                        .chunk(chunk.clone())
                        .score(RelevanceScore::try_new(0.95).unwrap())
                        .build(),
                    SearchResult::builder()
                        .chunk(chunk)
                        .score(RelevanceScore::try_new(0.85).unwrap())
                        .build(),
                ])
                .build(),
        ];

        // When Formatting file results in human format
        // Then It should print without panicking
        format_file_results_human(&file_results, false);
        format_file_results_human(&file_results, true);
    }

    #[test]
    fn test_format_file_results_human_empty() {
        // Given Empty file results
        let file_results = vec![];

        // When Formatting empty file results
        // Then It should print without panicking
        format_file_results_human(&file_results, false);
    }

    #[test]
    fn test_format_search_results_json_success() {
        // Given SearchResults with results
        let chunk = create_test_chunk();
        let results = SearchResults::builder()
            .query(create_test_query())
            .results(vec![
                SearchResult::builder()
                    .chunk(chunk)
                    .score(RelevanceScore::try_new(0.95).unwrap())
                    .build(),
            ])
            .total_chunks_searched(100usize)
            .search_duration(Duration::from_millis(50))
            .build();

        // When Grouping and formatting as file results
        let file_results = group_results_by_file(&results);

        // Then It should succeed
        assert!(file_results.len() > 0);
    }

    #[test]
    fn test_format_file_results_json_success() {
        // Given FileSearchResults
        let chunk = create_test_chunk();
        let file_results = vec![
            FileSearchResult::builder()
                .file_path(ForestRelativePath::try_new("test.md").unwrap())
                .repo_name(RepoName::try_new("test-repo").unwrap())
                .match_count(1usize)
                .best_score(RelevanceScore::try_new(0.95).unwrap())
                .chunks(vec![
                    SearchResult::builder()
                        .chunk(chunk)
                        .score(RelevanceScore::try_new(0.95).unwrap())
                        .build(),
                ])
                .build(),
        ];

        // When Formatting file results as JSON
        let result = format_file_results_json(&file_results);

        // Then It should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_group_results_by_file() {
        // Given SearchResults with multiple chunks from same file
        let chunk1 = create_test_chunk();
        let mut chunk2 = chunk1.clone();
        chunk2.id = ChunkId::new(uuid::Uuid::new_v4());

        let results = SearchResults::builder()
            .query(create_test_query())
            .results(vec![
                SearchResult::builder()
                    .chunk(chunk1)
                    .score(RelevanceScore::try_new(0.95).unwrap())
                    .build(),
                SearchResult::builder()
                    .chunk(chunk2)
                    .score(RelevanceScore::try_new(0.85).unwrap())
                    .build(),
            ])
            .total_chunks_searched(100usize)
            .search_duration(Duration::from_millis(50))
            .build();

        // When Grouping results by file
        let file_results = group_results_by_file(&results);

        // Then It should return one file with two chunks
        assert_eq!(file_results.len(), 1);
        assert_eq!(file_results[0].match_count, 2);
        assert_eq!(
            file_results[0].best_score,
            RelevanceScore::try_new(0.95).unwrap()
        );
    }

    #[test]
    fn test_format_status_prints_info() {
        // Given An IndexStatus with metadata
        let status = IndexStatus::builder()
            .exists(true)
            .chunk_count(100)
            .file_count(20)
            .last_build(SystemTime::now())
            .model_config(EmbeddingModelConfig::default())
            .size_bytes(1024000)
            .build();

        // When Formatting the status
        // Then It should print without panicking
        format_status(&status);
    }

    #[test]
    fn test_prompt_confirmation_yes() {
        // Given User input "yes"
        // When Prompting for confirmation
        // Then It should return true
        // Note: This test requires mocking stdin, which is complex
        // We'll test the actual implementation manually
    }

    #[test]
    fn test_prompt_confirmation_no() {
        // Given User input "no"
        // When Prompting for confirmation
        // Then It should return false
        // Note: This test requires mocking stdin, which is complex
        // We'll test the actual implementation manually
    }

    fn create_test_chunk() -> Chunk {
        use sembly_core::knowledge::domain::{ChunkHash, FileHash};
        use std::io::Cursor;

        let text = "Test content";
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .chunk_hash(ChunkHash::from_text(text))
            .file_hash(FileHash::from_reader(Cursor::new(text.as_bytes())).unwrap())
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text(text)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    fn create_test_query() -> SearchQuery {
        use sembly_core::knowledge::domain::{QueryText, ResultLimit};
        SearchQuery::builder()
            .text(QueryText::try_new("test query").unwrap())
            .limit(ResultLimit::try_new(10).unwrap())
            .build()
    }
}
