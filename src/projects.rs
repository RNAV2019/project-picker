pub fn fuzzy_match(query: &str, text: &str) -> bool {
    let query = query.to_lowercase();
    let text = text.to_lowercase();
    let mut query_chars = query.chars();
    let mut current = match query_chars.next() {
        None => return true,
        Some(c) => c,
    };
    for ch in text.chars() {
        if ch == current {
            match query_chars.next() {
                None => return true,
                Some(c) => current = c,
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query_matches_everything() {
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn test_exact_match() {
        assert!(fuzzy_match("foo", "foo"));
    }

    #[test]
    fn test_subsequence_match() {
        assert!(fuzzy_match("pck", "project-picker"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(fuzzy_match("PCK", "project-picker"));
    }

    #[test]
    fn test_no_match() {
        assert!(!fuzzy_match("xyz", "project-picker"));
    }

    #[test]
    fn test_partial_match_fails() {
        assert!(!fuzzy_match("abc", "ab"));
    }
}
