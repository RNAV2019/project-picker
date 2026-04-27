pub fn tilde_collapse(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}{}", home, &path[1..])
    } else if path == "~" {
        std::env::var("HOME").unwrap_or_default()
    } else {
        path.to_string()
    }
}

pub fn get_suggestions(typed: &str) -> Vec<String> {
    if typed.is_empty() {
        return vec![];
    }
    let expanded = expand_tilde(typed);
    let pattern = if typed.ends_with('/') {
        format!("{}*", expanded)
    } else {
        format!("{}*/", expanded)
    };
    let mut matches: Vec<String> = glob::glob(&pattern)
        .unwrap_or_else(|_| glob::glob("/dev/null").unwrap())
        .filter_map(|entry| entry.ok())
        .filter(|p| p.is_dir())
        .map(|p| tilde_collapse(p.to_string_lossy().as_ref()))
        .take(20)
        .collect();
    matches.sort();
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input_returns_empty() {
        assert_eq!(get_suggestions(""), vec![] as Vec<String>);
    }

    #[test]
    fn test_tilde_collapse() {
        let home = std::env::var("HOME").unwrap();
        let full = format!("{}/projects", home);
        assert_eq!(tilde_collapse(&full), "~/projects");
    }

    #[test]
    fn test_suggestions_from_real_tmp() {
        let results = get_suggestions("/tmp/");
        for r in &results {
            assert!(r.starts_with('/') || r.starts_with('~'));
        }
        assert!(results.len() <= 20);
    }
}
