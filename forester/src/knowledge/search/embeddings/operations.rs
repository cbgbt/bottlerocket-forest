//! Vector operations for embeddings
//!
//! Provides mathematical operations on embedding vectors for semantic similarity
//! computation. These operations are used by the semantic search engine to rank
//! results based on vector similarity.

use crate::knowledge::domain::Embedding;

/// Vector operations for embeddings
pub trait VectorOps {
    /// Calculate cosine similarity between two embeddings
    ///
    /// Returns a value between -1.0 (opposite) and 1.0 (identical).
    /// Higher values indicate greater similarity.
    fn cosine_similarity(&self, other: &Self) -> f32;

    /// Calculate the L2 (Euclidean) norm of the embedding
    fn magnitude(&self) -> f32;

    /// Normalize the embedding to unit length
    fn normalize(&self) -> Self;

    /// Calculate dot product with another embedding
    fn dot_product(&self, other: &Self) -> f32;
}

impl VectorOps for Embedding {
    fn cosine_similarity(&self, other: &Self) -> f32 {
        let dot = self.dot_product(other);
        let mag_product = self.magnitude() * other.magnitude();
        dot / mag_product
    }

    fn magnitude(&self) -> f32 {
        self.dot_product(self).sqrt()
    }

    fn normalize(&self) -> Self {
        let mag = self.magnitude();
        let normalized: Vec<f32> = self.as_ref().iter().map(|&x| x / mag).collect();
        Embedding::try_new(normalized).expect("normalized vector cannot be empty")
    }

    fn dot_product(&self, other: &Self) -> f32 {
        self.as_ref()
            .iter()
            .zip(other.as_ref().iter())
            .map(|(a, b)| a * b)
            .sum()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_dot_product_orthogonal_vectors() {
        // Given Two orthogonal vectors
        let v1 = Embedding::try_new(vec![1.0, 0.0, 0.0]).unwrap();
        let v2 = Embedding::try_new(vec![0.0, 1.0, 0.0]).unwrap();

        // When Computing dot product
        let result = v1.dot_product(&v2);

        // Then Result should be zero
        assert!((result - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_parallel_vectors() {
        // Given Two parallel vectors
        let v1 = Embedding::try_new(vec![1.0, 2.0, 3.0]).unwrap();
        let v2 = Embedding::try_new(vec![2.0, 4.0, 6.0]).unwrap();

        // When Computing dot product
        let result = v1.dot_product(&v2);

        // Then Result should be 1*2 + 2*4 + 3*6 = 28
        assert!((result - 28.0).abs() < 1e-6);
    }

    #[test]
    fn test_magnitude_unit_vector() {
        // Given A unit vector
        let v = Embedding::try_new(vec![1.0, 0.0, 0.0]).unwrap();

        // When Computing magnitude
        let result = v.magnitude();

        // Then Result should be 1.0
        assert!((result - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_magnitude_general_vector() {
        // Given A vector [3, 4]
        let v = Embedding::try_new(vec![3.0, 4.0]).unwrap();

        // When Computing magnitude
        let result = v.magnitude();

        // Then Result should be 5.0 (3-4-5 triangle)
        assert!((result - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_creates_unit_vector() {
        // Given A non-unit vector
        let v = Embedding::try_new(vec![3.0, 4.0]).unwrap();

        // When Normalizing the vector
        let normalized = v.normalize();

        // Then Magnitude should be 1.0
        assert!((normalized.magnitude() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_preserves_direction() {
        // Given A vector
        let v = Embedding::try_new(vec![3.0, 4.0]).unwrap();

        // When Normalizing the vector
        let normalized = v.normalize();

        // Then Components should be [0.6, 0.8]
        let components: &Vec<f32> = normalized.as_ref();
        assert!((components[0] - 0.6).abs() < 1e-6);
        assert!((components[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        // Given Two identical vectors
        let v1 = Embedding::try_new(vec![1.0, 2.0, 3.0]).unwrap();
        let v2 = Embedding::try_new(vec![1.0, 2.0, 3.0]).unwrap();

        // When Computing cosine similarity
        let result = v1.cosine_similarity(&v2);

        // Then Result should be 1.0
        assert!((result - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        // Given Two orthogonal vectors
        let v1 = Embedding::try_new(vec![1.0, 0.0, 0.0]).unwrap();
        let v2 = Embedding::try_new(vec![0.0, 1.0, 0.0]).unwrap();

        // When Computing cosine similarity
        let result = v1.cosine_similarity(&v2);

        // Then Result should be 0.0
        assert!((result - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        // Given Two opposite vectors
        let v1 = Embedding::try_new(vec![1.0, 2.0, 3.0]).unwrap();
        let v2 = Embedding::try_new(vec![-1.0, -2.0, -3.0]).unwrap();

        // When Computing cosine similarity
        let result = v1.cosine_similarity(&v2);

        // Then Result should be -1.0
        assert!((result - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_scaled_vectors() {
        // Given Two vectors that differ only in magnitude
        let v1 = Embedding::try_new(vec![1.0, 2.0, 3.0]).unwrap();
        let v2 = Embedding::try_new(vec![2.0, 4.0, 6.0]).unwrap();

        // When Computing cosine similarity
        let result = v1.cosine_similarity(&v2);

        // Then Result should be 1.0 (same direction)
        assert!((result - 1.0).abs() < 1e-6);
    }
}
