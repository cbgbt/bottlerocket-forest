//! Constants for the knowledge indexing system.

/// Embedding vector dimension for all-MiniLM-L6-v2 model
pub const EMBEDDING_DIM: usize = 384;

/// Default tokenizer model for chunking and embeddings
pub const DEFAULT_TOKENIZER_MODEL: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Default maximum tokens per chunk (model's context window limit)
pub const DEFAULT_MAX_CHUNK_TOKENS: usize = 256;

/// Default overlap percentage between chunks (0.0 to 1.0)
pub const DEFAULT_CHUNK_OVERLAP_PERCENTAGE: f32 = 0.15;

/// Default computed overlap in tokens
pub const DEFAULT_CHUNK_OVERLAP_TOKENS: usize =
    (DEFAULT_MAX_CHUNK_TOKENS as f32 * DEFAULT_CHUNK_OVERLAP_PERCENTAGE) as usize;

/// Maximum number of threads for parallel file processing during indexing
///
/// Limits rayon parallelism to prevent CPU saturation. ONNX Runtime (used by
/// fastembed) also uses multiple threads per embedding generation, so limiting
/// file-level parallelism prevents thread oversubscription.
pub const MAX_INDEXING_THREADS: usize = 4;
