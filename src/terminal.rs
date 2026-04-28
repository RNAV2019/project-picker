use std::process::Command;

pub fn open_terminal(path: &str) {
    let abs = expand_tilde(path);
    let _ = Command::new("uwsm-app")
        .args(["--", "ghostty", &format!("--working-directory={}", abs)])
        .spawn();
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}{}", home, &path[1..])
    } else {
        path.to_string()
    }
}
