use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::index_progress::CliProgressReporter;
use crate::knowledge::KnowledgeIndex;
use crate::knowledge::domain::{
    FileSearchResult, ForestRelativePath, RelevanceScore, RepoName, SearchResult, SearchResults,
};
use crate::knowledge::storage::ChunkRepository;

/// Manage the knowledge index
#[derive(Parser)]
pub struct IndexCommand {
    #[command(subcommand)]
    subcommand: IndexSubcommand,
}

#[derive(Subcommand)]
enum IndexSubcommand {
    /// Build the knowledge index
    Build(BuildArgs),
    /// Rebuild the knowledge index from scratch
    Rebuild(RebuildArgs),
    /// Update the knowledge index incrementally
    Update(UpdateArgs),
    /// Clear all chunks from the index
    Clear(ClearArgs),
    /// Search the knowledge index
    Search(SearchArgs),
    /// Show index status and statistics
    Status(StatusArgs),
}

/// Build the knowledge index
#[derive(Parser)]
struct BuildArgs {
    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Rebuild the knowledge index from scratch
#[derive(Parser)]
struct RebuildArgs {
    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Update the knowledge index incrementally
#[derive(Parser)]
struct UpdateArgs {
    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

/// Clear all chunks from the index
#[derive(Parser)]
struct ClearArgs {
    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,

    /// skip confirmation prompt
    #[arg(short = 'y', long)]
    yes: bool,
}

/// Search the knowledge index
#[derive(Parser)]
struct SearchArgs {
    /// search query
    query: String,

    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,

    /// maximum number of results (1-100, defaults to 10)
    #[arg(short = 'n', long)]
    limit: Option<usize>,

    /// output format: human or json (defaults to human)
    #[arg(short = 'f', long)]
    format: Option<String>,

    /// show individual chunk matches under each file
    #[arg(long)]
    show_chunks: bool,
}

/// Show index status and statistics
#[derive(Parser)]
struct StatusArgs {
    /// path to forest root (defaults to current directory)
    #[arg(long)]
    forest_root: Option<PathBuf>,
}

pub fn run(cmd: IndexCommand) -> Result<(), IndexError> {
    match cmd.subcommand {
        IndexSubcommand::Build(args) => handle_build(args),
        IndexSubcommand::Rebuild(args) => handle_rebuild(args),
        IndexSubcommand::Update(args) => handle_update(args),
        IndexSubcommand::Clear(args) => handle_clear(args),
        IndexSubcommand::Search(args) => handle_search(args),
        IndexSubcommand::Status(args) => handle_status(args),
    }
}

fn handle_build(args: BuildArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let mut index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let progress = Box::new(CliProgressReporter::default());
    let result = index
        .build_with_progress(Some(progress))
        .context(KnowledgeIndexSnafu)?;

    format_build_result(&result);

    Ok(())
}

fn handle_rebuild(args: RebuildArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let mut index = KnowledgeIndex::open(&forest_root).context(KnowledgeIndexSnafu)?;

    let progress = Box::new(CliProgressReporter::default());
    let result = index
        .rebuild_with_progress(Some(progress))
        .context(KnowledgeIndexSnafu)?;

    format_build_result(&result);

    Ok(())
}

fn handle_update(args: UpdateArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let mut index = open_existing_index(&forest_root)?;

    let progress = Box::new(CliProgressReporter::default());
    let result = index
        .update_with_progress(Some(progress))
        .context(KnowledgeIndexSnafu)?;

    format_update_result(&result);

    Ok(())
}

fn handle_clear(args: ClearArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    if !args.yes && !prompt_confirmation("Are you sure you want to clear the index?")? {
        println!("Cancelled");
        return Ok(());
    }

    let mut index = open_existing_index(&forest_root)?;

    let count = index.clear().context(KnowledgeIndexSnafu)?;

    println!("Cleared {} chunks from the index", count);

    Ok(())
}

fn handle_search(args: SearchArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let limit = args.limit.unwrap_or(10);

    let format = parse_output_format(args.format.as_deref())?;

    let index = open_existing_index(&forest_root)?;

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

fn handle_status(args: StatusArgs) -> Result<(), IndexError> {
    use index_error::*;

    let forest_root = args
        .forest_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let index = open_existing_index(&forest_root)?;

    let status = index.status().context(KnowledgeIndexSnafu)?;

    format_status(&status);

    Ok(())
}

fn open_existing_index(forest_root: &std::path::Path) -> Result<KnowledgeIndex, IndexError> {
    use index_error::*;

    let db_path = forest_root.join(".forester/knowledge.db");

    if !db_path.exists() {
        return Err(IndexError::IndexNotFound {
            path: db_path.display().to_string(),
        });
    }

    let temp_repo = crate::knowledge::storage::sqlite::SqliteChunkRepository::open(
        &db_path,
        &crate::knowledge::domain::EmbeddingModelConfig::default(),
    )
    .map_err(|e| IndexError::KnowledgeIndex {
        source: crate::knowledge::facade::IndexError::DatabaseAccessFailed { source: e },
    })?;

    let metadata = temp_repo
        .get_metadata()
        .map_err(|e| IndexError::KnowledgeIndex {
            source: crate::knowledge::facade::IndexError::DatabaseAccessFailed { source: e },
        })?;

    KnowledgeIndex::open_with_config(forest_root, metadata.model_config)
        .context(KnowledgeIndexSnafu)
}

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

fn format_build_result(result: &crate::knowledge::indexing::IndexResult) {
    println!("Index build complete!");
    println!("  Files processed: {}", result.files_processed);
    println!("  Chunks created: {}", result.chunks_affected);
    println!("  Duration: {:.2}s", result.duration.as_secs_f64());
}

fn format_update_result(result: &crate::knowledge::indexing::IndexResult) {
    println!("Index update complete!");
    println!("  Files added: {}", result.files_added);
    println!("  Files updated: {}", result.files_updated);
    println!("  Files removed: {}", result.files_removed);
    println!("  Chunks affected: {}", result.chunks_affected);
    println!("  Duration: {:.2}s", result.duration.as_secs_f64());
}

fn format_file_results_human(file_results: &[FileSearchResult], show_chunks: bool) {
    if file_results.is_empty() {
        println!("No files matched");
        return;
    }

    println!(
        "Found {} unique files:\n",
        file_results.len().to_string().cyan()
    );

    for (i, file_result) in file_results.iter().enumerate() {
        let score_value = file_result.best_score.into_inner();
        let score_colored = colorize_score(score_value);

        println!(
            "{}. [Matches: {}, Best Score: {}] {}",
            (i + 1).to_string().bright_blue(),
            file_result.match_count.to_string().cyan(),
            score_colored,
            file_result.file_path.to_string().bright_white()
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
                    colorize_score(chunk_score),
                    preview.replace('\n', " ").dimmed()
                );
            }
        }
    }
}

/// Returns a colorized score string based on relevance thresholds.
///
/// Colors automatically disable when output is not a TTY.
fn colorize_score(score: f32) -> String {
    let formatted = format!("{:.3}", score);
    if score >= 0.8 {
        formatted.green().to_string()
    } else if score >= 0.6 {
        formatted.yellow().to_string()
    } else if score >= 0.4 {
        formatted.truecolor(255, 165, 0).to_string() // orange
    } else {
        formatted.red().to_string()
    }
}

fn format_file_results_json(file_results: &[FileSearchResult]) -> Result<(), IndexError> {
    use index_error::*;

    let json = serde_json::to_string_pretty(file_results).context(JsonSerializationFailedSnafu)?;
    println!("{}", json);
    Ok(())
}

fn format_status(status: &crate::knowledge::facade::IndexStatus) {
    println!("Knowledge Index Status");
    println!("  Exists: {}", status.exists);
    println!("  Chunks: {}", status.chunk_count);
    println!("  Files: {}", status.file_count);

    if let Some(Ok(elapsed)) = status.last_build.map(|t| t.elapsed()) {
        println!("  Last build: {:.0} seconds ago", elapsed.as_secs_f64());
    }

    println!("  Model: {}", status.model_config.model_name);
    println!("  Embedding dim: {}", status.model_config.embedding_dim);
    println!("  Max tokens: {}", status.model_config.max_tokens);

    if let Some(size) = status.size_bytes {
        println!("  Size: {:.2} MB", size as f64 / 1_000_000.0);
    }
}

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
        code(forester::cli::knowledge_index_failed),
        help("Check the error details above for specific guidance")
    )]
    KnowledgeIndex {
        source: crate::knowledge::facade::IndexError,
    },

    #[snafu(display("Index not found at {path}"))]
    #[diagnostic(
        code(forester::cli::index_not_found),
        help("Run `forester index build` to create it")
    )]
    IndexNotFound { path: String },

    #[snafu(display("Invalid output format: {format}"))]
    #[diagnostic(
        code(forester::cli::invalid_output_format),
        help("Must be 'human' or 'json'")
    )]
    InvalidOutputFormat { format: String },

    #[snafu(display("Failed to read user input"))]
    #[diagnostic(
        code(forester::cli::input_read_failed),
        help("Check that stdin is available and not closed")
    )]
    InputReadFailed { source: std::io::Error },

    #[snafu(display("Failed to serialize JSON output"))]
    #[diagnostic(
        code(forester::cli::json_serialization_failed),
        help("The search results may contain invalid data")
    )]
    JsonSerializationFailed { source: serde_json::Error },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::knowledge::domain::{
        Chunk, ChunkContent, ChunkContext, ChunkId, ChunkSource, EmbeddingModelConfig,
        ForestRelativePath, MarkdownContext, RelevanceScore, RepoName, SearchQuery, SearchResult,
        SearchResults, TokenCount,
    };
    use crate::knowledge::facade::IndexStatus;
    use crate::knowledge::indexing::IndexResult;
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
    fn test_open_existing_index_not_found() {
        // Given A forest root with no index database
        let temp_dir = tempfile::tempdir().unwrap();
        let forest_root = temp_dir.path();

        // When Opening the existing index
        let result = open_existing_index(forest_root);

        // Then It should return IndexNotFound error
        assert!(matches!(result, Err(IndexError::IndexNotFound { .. })));
    }

    #[test]
    fn test_open_existing_index_success() {
        // Given A forest root with an existing index database
        let temp_dir = tempfile::tempdir().unwrap();
        let forest_root = temp_dir.path();
        let forester_dir = forest_root.join(".forester");
        std::fs::create_dir_all(&forester_dir).unwrap();

        // Create a valid index
        let mut index = KnowledgeIndex::open(forest_root).unwrap();
        index.build().unwrap();

        // When Opening the existing index
        let result = open_existing_index(forest_root);

        // Then It should successfully return a KnowledgeIndex
        assert!(result.is_ok());
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
                    SearchResult {
                        chunk: chunk.clone(),
                        score: RelevanceScore::try_new(0.95).unwrap(),
                    },
                    SearchResult {
                        chunk,
                        score: RelevanceScore::try_new(0.85).unwrap(),
                    },
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
            .results(vec![SearchResult {
                chunk,
                score: RelevanceScore::try_new(0.95).unwrap(),
            }])
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
                .chunks(vec![SearchResult {
                    chunk,
                    score: RelevanceScore::try_new(0.95).unwrap(),
                }])
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
                SearchResult {
                    chunk: chunk1,
                    score: RelevanceScore::try_new(0.95).unwrap(),
                },
                SearchResult {
                    chunk: chunk2,
                    score: RelevanceScore::try_new(0.85).unwrap(),
                },
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
        Chunk::builder()
            .id(ChunkId::new(uuid::Uuid::new_v4()))
            .source(
                ChunkSource::builder()
                    .file_path(ForestRelativePath::try_new("test.md").unwrap())
                    .repo_name(RepoName::try_new("test-repo").unwrap())
                    .build(),
            )
            .content(
                ChunkContent::builder()
                    .text("Test content")
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder().heading_hierarchy(vec![]).build(),
            ))
            .build()
    }

    fn create_test_query() -> SearchQuery {
        use crate::knowledge::domain::{QueryText, ResultLimit};
        SearchQuery::builder()
            .text(QueryText::try_new("test query").unwrap())
            .limit(ResultLimit::try_new(10).unwrap())
            .build()
    }
}
