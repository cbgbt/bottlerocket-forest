use std::collections::BTreeMap;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is", "it",
    "its", "of", "on", "that", "the", "to", "was", "will", "with",
];

/// Calculates BM25 term frequencies from text
///
/// Tokenizes text by splitting on whitespace and punctuation, converts to lowercase,
/// removes stopwords, and counts term frequencies.
pub fn calculate_bm25_terms(text: &str) -> BTreeMap<String, u32> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter_map(|token| {
            let token = token.to_lowercase();
            if token.is_empty() || STOPWORDS.contains(&token.as_str()) {
                None
            } else {
                Some(token)
            }
        })
        .fold(BTreeMap::new(), |mut map, term| {
            *map.entry(term).or_insert(0) += 1;
            map
        })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_empty_text() {
        // Given empty text
        let text = "";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then no terms are returned
        assert!(terms.is_empty());
    }

    #[test]
    fn test_stopwords_removed() {
        // Given text with only stopwords
        let text = "the a an and is";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then no terms are returned
        assert!(terms.is_empty());
    }

    #[test]
    fn test_case_normalization() {
        // Given text with mixed case
        let text = "Rust RUST rust";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then all variants are counted as one term
        assert_eq!(terms.len(), 1);
        assert_eq!(terms.get("rust"), Some(&3));
    }

    #[test]
    fn test_punctuation_splitting() {
        // Given text with punctuation
        let text = "hello,world!test";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then punctuation splits tokens
        assert_eq!(terms.len(), 3);
        assert_eq!(terms.get("hello"), Some(&1));
        assert_eq!(terms.get("world"), Some(&1));
        assert_eq!(terms.get("test"), Some(&1));
    }

    #[test]
    fn test_term_frequency_counting() {
        // Given text with repeated terms
        let text = "rust programming rust language programming";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then frequencies are counted correctly
        assert_eq!(terms.get("rust"), Some(&2));
        assert_eq!(terms.get("programming"), Some(&2));
        assert_eq!(terms.get("language"), Some(&1));
    }

    #[test]
    fn test_mixed_content() {
        // Given realistic text with stopwords, punctuation, and content
        let text = "The Rust programming language is fast and safe.";

        // When calculating BM25 terms
        let terms = calculate_bm25_terms(text);

        // Then only content words are indexed
        assert!(!terms.contains_key("the"));
        assert!(!terms.contains_key("is"));
        assert!(!terms.contains_key("and"));
        assert_eq!(terms.get("rust"), Some(&1));
        assert_eq!(terms.get("programming"), Some(&1));
        assert_eq!(terms.get("language"), Some(&1));
        assert_eq!(terms.get("fast"), Some(&1));
        assert_eq!(terms.get("safe"), Some(&1));
    }
}
