use std::process::Command;

pub fn open_terminal(path: &str) {
    let abs = crate::paths::expand_tilde(path);
    let _ = Command::new("ghostty")
        .arg(format!("--working-directory={}", abs))
        .spawn();
}
