//! Content-addressed hash types for deduplication
//!
//! Provides SHA-256 based hashing for files and chunks to enable
//! content-addressed storage where identical content shares embeddings
//! across multiple contexts.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{self, Read};

use super::Chunk;

/// SHA-256 hash of file content for content-addressed storage
///
/// Files with identical content produce identical hashes, enabling
/// embedding reuse across contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileHash([u8; 32]);

impl FileHash {
    /// Creates a FileHash from a 32-byte array
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Computes hash from a reader (streaming, memory-efficient)
    pub fn from_reader(mut reader: impl Read) -> io::Result<Self> {
        let mut hasher = Sha256::new();
        io::copy(&mut reader, &mut hasher)?;
        Ok(Self(hasher.finalize().into()))
    }

    /// Returns the hash as a byte slice
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// SHA-256 hash of chunk content for content-addressed storage
///
/// Chunks with identical content produce identical hashes, enabling
/// embedding reuse when the same text appears in different files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkHash([u8; 32]);

impl ChunkHash {
    /// Creates a ChunkHash from a 32-byte array
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Computes hash from text content
    pub fn from_text(text: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        Self(hasher.finalize().into())
    }

    /// Computes hash from a Chunk's text content
    pub fn from_chunk(chunk: &Chunk) -> Self {
        Self::from_text(&chunk.content.text)
    }

    /// Returns the hash as a byte slice
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ChunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use uuid::Uuid;

    use super::*;
    use crate::knowledge::domain::{
        ChunkContent, ChunkContext, ChunkId, ChunkSource, ForestRelativePath, HeadingText,
        MarkdownContext, RepoName, TokenCount,
    };

    fn make_chunk(text: &str) -> Chunk {
        Chunk::builder()
            .id(ChunkId::new(Uuid::new_v4()))
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
                    .text(text)
                    .token_count(TokenCount::try_new(10).unwrap())
                    .build(),
            )
            .context(ChunkContext::Markdown(
                MarkdownContext::builder()
                    .heading_hierarchy(vec![HeadingText::try_new("Test").unwrap()])
                    .build(),
            ))
            .build()
    }

    #[test]
    fn file_hash_same_content_produces_same_hash() {
        // Given identical file content
        let content = b"Hello, world!";

        // When computing hashes
        let hash1 = FileHash::from_reader(Cursor::new(content)).unwrap();
        let hash2 = FileHash::from_reader(Cursor::new(content)).unwrap();

        // Then hashes should be identical
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn file_hash_different_content_produces_different_hash() {
        // Given different file content
        let content1 = b"Hello, world!";
        let content2 = b"Goodbye, world!";

        // When computing hashes
        let hash1 = FileHash::from_reader(Cursor::new(content1)).unwrap();
        let hash2 = FileHash::from_reader(Cursor::new(content2)).unwrap();

        // Then hashes should differ
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn file_hash_is_deterministic() {
        // Given the same content
        let content = b"deterministic test";

        // When computing hash multiple times
        let hash1 = FileHash::from_reader(Cursor::new(content)).unwrap();
        let hash2 = FileHash::from_reader(Cursor::new(content)).unwrap();
        let hash3 = FileHash::from_reader(Cursor::new(content)).unwrap();

        // Then all hashes should be identical
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn file_hash_display_shows_hex() {
        // Given a file hash
        let bytes = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xab, 0xcd, 0xef,
        ];
        let hash = FileHash::new(bytes);

        // When formatting as string
        let display = format!("{}", hash);

        // Then should show lowercase hex representation
        assert_eq!(
            display,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn chunk_hash_same_content_produces_same_hash() {
        // Given identical chunk content
        let chunk1 = make_chunk("This is a chunk of text");
        let chunk2 = make_chunk("This is a chunk of text");

        // When computing hashes
        let hash1 = ChunkHash::from_chunk(&chunk1);
        let hash2 = ChunkHash::from_chunk(&chunk2);

        // Then hashes should be identical
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn chunk_hash_different_content_produces_different_hash() {
        // Given different chunk content
        let chunk1 = make_chunk("This is chunk one");
        let chunk2 = make_chunk("This is chunk two");

        // When computing hashes
        let hash1 = ChunkHash::from_chunk(&chunk1);
        let hash2 = ChunkHash::from_chunk(&chunk2);

        // Then hashes should differ
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn chunk_hash_is_deterministic() {
        // Given the same content
        let chunk = make_chunk("deterministic chunk test");

        // When computing hash multiple times
        let hash1 = ChunkHash::from_chunk(&chunk);
        let hash2 = ChunkHash::from_chunk(&chunk);
        let hash3 = ChunkHash::from_chunk(&chunk);

        // Then all hashes should be identical
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn chunk_hash_display_shows_hex() {
        // Given a chunk hash
        let bytes = [
            0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54,
            0x32, 0x10, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0xfe, 0xdc, 0xba, 0x98,
            0x76, 0x54, 0x32, 0x10,
        ];
        let hash = ChunkHash::new(bytes);

        // When formatting as string
        let display = format!("{}", hash);

        // Then should show lowercase hex representation
        assert_eq!(
            display,
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
        );
    }
}
