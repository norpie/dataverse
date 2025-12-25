//! Default fuzzy filtering using nucleo-matcher.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Result of a fuzzy filter operation.
#[derive(Debug, Clone)]
pub struct FilterMatch {
    /// Index of the matched item in the original list.
    pub index: usize,
    /// Match score (higher is better).
    pub score: u32,
}

/// Default fuzzy filter using nucleo-matcher.
///
/// Returns matches sorted by score (highest first).
/// Empty query returns all items with score 0.
///
/// # Example
///
/// ```ignore
/// let items = vec!["apple", "banana", "apricot"];
/// let labels: Vec<String> = items.iter().map(|s| s.to_string()).collect();
/// let matches = fuzzy_filter("ap", &labels);
/// // Returns: apricot (highest score), apple
/// ```
pub fn fuzzy_filter(query: &str, items: &[String]) -> Vec<FilterMatch> {
    // Empty query returns all items
    if query.is_empty() {
        return items
            .iter()
            .enumerate()
            .map(|(index, _)| FilterMatch { index, score: 0 })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut matches: Vec<FilterMatch> = items
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let mut buf = Vec::new();
            let haystack = Utf32Str::new(label, &mut buf);
            pattern
                .score(haystack, &mut matcher)
                .map(|score| FilterMatch { index, score })
        })
        .collect();

    // Sort by score descending (higher score = better match)
    matches.sort_by(|a, b| b.score.cmp(&a.score));

    matches
}
