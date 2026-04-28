use std::path::{Path, PathBuf};

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

pub fn recents_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/project-picker/recents.json")
}

pub fn load_recents_from(path: &Path) -> Vec<String> {
    let Ok(data) = std::fs::read(path) else { return vec![] };
    let Ok(parsed) = serde_json::from_slice::<Vec<serde_json::Value>>(&data) else { return vec![] };
    parsed.into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

pub fn save_recents_to(path: &Path, recents: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(recents).unwrap();
    std::fs::write(&tmp, &data).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

pub fn load_recents() -> Vec<String> {
    load_recents_from(&recents_path())
}

pub fn save_recents(recents: &[String]) {
    save_recents_to(&recents_path(), recents);
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

    #[test]
    fn test_load_missing_file_returns_empty() {
        let result = load_recents_from(Path::new("/nonexistent/path/recents.json"));
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        let paths = vec!["~/projects/foo".to_string(), "~/projects/bar".to_string()];
        save_recents_to(&path, &paths);
        let loaded = load_recents_from(&path);
        assert_eq!(loaded, paths);
    }

    #[test]
    fn test_load_malformed_json_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        std::fs::write(&path, b"not json").unwrap();
        let result = load_recents_from(&path);
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_save_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recents.json");
        save_recents_to(&path, &["~/foo".to_string()]);
        assert!(path.exists());
    }
}
