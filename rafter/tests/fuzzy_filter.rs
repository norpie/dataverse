use rafter::widgets::autocomplete::fuzzy_filter;

#[test]
fn test_empty_query_returns_all() {
    let items = vec!["apple".to_string(), "banana".to_string()];
    let matches = fuzzy_filter("", &items);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].index, 0);
    assert_eq!(matches[1].index, 1);
}

#[test]
fn test_fuzzy_matching() {
    let items = vec![
        "apple".to_string(),
        "banana".to_string(),
        "apricot".to_string(),
    ];
    let matches = fuzzy_filter("ap", &items);
    assert_eq!(matches.len(), 2);
    // Both apple and apricot match "ap"
    let indices: Vec<usize> = matches.iter().map(|m| m.index).collect();
    assert!(indices.contains(&0)); // apple
    assert!(indices.contains(&2)); // apricot
}

#[test]
fn test_no_matches() {
    let items = vec!["apple".to_string(), "banana".to_string()];
    let matches = fuzzy_filter("xyz", &items);
    assert!(matches.is_empty());
}

#[test]
fn test_case_insensitive() {
    let items = vec!["Apple".to_string(), "BANANA".to_string()];
    let matches = fuzzy_filter("apple", &items);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].index, 0);
}
