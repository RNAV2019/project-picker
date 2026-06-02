# Mycelium App Launcher — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fork `project-picker` into `mycelium`, a Wayland app launcher that reads system `.desktop` files, ranks results by frecency, and includes a live scientific calculator mode triggered by `=`.

**Architecture:** The egui + wgpu + winit daemon architecture is preserved unchanged. The domain layer is replaced: `projects.rs`/`terminal.rs`/`paths.rs`/`assets.rs` are deleted and replaced by `apps.rs` (XDG desktop file scanning + frecency), `calculator.rs` (live expression evaluation), and `launcher.rs` (app spawning). `icons.rs` is rewritten to resolve icon names via the system icon theme using `linicon`. `app.rs` replaces pinned/recent project state with `Vec<AppEntry>` + `FrecencyStore` + calculator result. `ui/list.rs` gets a new `app_row` and `calc_result_row` in place of the old project/action rows.

**Tech Stack:** Rust 2021, egui 0.29, wgpu 22, winit 0.30, evalexpr 11 (calculator), linicon 0.4 (icon theme), wl-clipboard-rs 0.8 (Wayland clipboard), serde/serde_json (frecency persistence)

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Create | `src/apps.rs` | XDG desktop file scanning, frecency store, fuzzy filter + ranking |
| Create | `src/calculator.rs` | Calculator mode detection + `evalexpr` expression evaluation |
| Create | `src/launcher.rs` | Exec= parsing, app spawning, Terminal=true handling |
| Rewrite | `src/icons.rs` | Icon name → path via `linicon`, existing rasterization reused |
| Rewrite | `src/app.rs` | App state: `Vec<AppEntry>`, `FrecencyStore`, `calc_result` |
| Rewrite | `src/ui/list.rs` | `app_row`, `calc_result_row`; remove old project/action rows |
| Modify | `src/ui/hints.rs` | Update hint labels (remove Pin/Remove, update Enter label) |
| Modify | `src/main.rs` | Socket path, config dir, binary name in messages |
| Modify | `src/daemon.rs` | Window title, socket path, SIGHUP flag |
| Modify | `Cargo.toml` | Rename crate, add evalexpr/linicon/wl-clipboard-rs, remove glob |
| Delete | `src/projects.rs` | Replaced by apps.rs |
| Delete | `src/terminal.rs` | Replaced by launcher.rs |
| Delete | `src/paths.rs` | Not needed |
| Delete | `src/assets.rs` + `src/assets/` | Not needed (system icons used) |

---

## Task 1: Bootstrap — Copy Project and Initialize Git

**Files:**
- Create: `/home/ryan/Projects/mycelium/` (populated from project-picker)

- [ ] **Step 1: Copy project-picker into mycelium**

```bash
cp -r /home/ryan/Projects/project-picker/. /home/ryan/Projects/mycelium/
cd /home/ryan/Projects/mycelium
rm -rf target .git
```

- [ ] **Step 2: Initialize git repo**

```bash
cd /home/ryan/Projects/mycelium
git init
git add -A
git commit -m "chore: initial fork from project-picker"
```

Expected: `git log --oneline` shows one commit.

---

## Task 2: Full Rename (project-picker → mycelium)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/daemon.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace the `[package]` and `[[bin]]` sections:

```toml
[package]
name = "mycelium"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mycelium"
path = "src/main.rs"
```

- [ ] **Step 2: Update socket path and messages in src/main.rs**

Change:
```rust
const SOCKET_PATH: &str = "/tmp/project-picker.sock";
```
to:
```rust
const SOCKET_PATH: &str = "/tmp/mycelium.sock";
```

Also change the error message:
```rust
eprintln!("mycelium: daemon did not start in time");
```

- [ ] **Step 3: Update socket path and window title in src/daemon.rs**

Change:
```rust
const SOCKET_PATH: &str = "/tmp/project-picker.sock";
```
to:
```rust
const SOCKET_PATH: &str = "/tmp/mycelium.sock";
```

Change the window attributes:
```rust
fn window_attrs() -> WindowAttributes {
    WindowAttributes::default()
        .with_title("Mycelium")
        .with_name("io.mycelium", "mycelium")
        .with_inner_size(LogicalSize::new(LOGICAL_WIDTH, LOGICAL_HEIGHT))
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(true)
}
```

- [ ] **Step 4: Verify it still builds**

```bash
cd /home/ryan/Projects/mycelium
cargo build 2>&1 | head -20
```

Expected: compiles (warnings OK, errors not OK).

- [ ] **Step 5: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add Cargo.toml src/main.rs src/daemon.rs
git commit -m "chore: rename project-picker → mycelium"
```

---

## Task 3: Cargo.toml — Update Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update the `[dependencies]` section**

Replace the full `[dependencies]` block:

```toml
[dependencies]
egui = "0.29"
egui-wgpu = "0.29"
wgpu = { version = "22", features = [] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"

evalexpr = "11"
linicon = "0.4"
wl-clipboard-rs = "0.8"

image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
ico = "0.3"
resvg = "0.42"
usvg = "0.42"
tiny-skia = "0.11"
pollster = "0.3"
egui-phosphor = { version = "0.7", features = ["regular"] }
egui-winit = { version = "0.29", default-features = false, features = ["wayland", "clipboard", "links"] }
winit = { version = "0.30", default-features = false, features = ["wayland", "rwh_06"] }

[dev-dependencies]
tempfile = "3"
```

Note: `glob = "0.3"` is removed (was used by paths.rs which we're deleting).

- [ ] **Step 2: Fetch new crates**

```bash
cd /home/ryan/Projects/mycelium
cargo fetch 2>&1 | tail -5
```

Expected: downloads evalexpr, linicon, wl-clipboard-rs and their deps without errors.

- [ ] **Step 3: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add Cargo.toml Cargo.lock
git commit -m "chore: swap deps — add evalexpr, linicon, wl-clipboard-rs; remove glob"
```

---

## Task 4: Create `src/apps.rs` — Desktop File Scanning + Frecency

**Files:**
- Create: `src/apps.rs`

This module owns the full application list and frecency state. It is the domain core of Mycelium.

- [ ] **Step 1: Write the failing tests first**

Create `src/apps.rs` with just the tests and empty stubs:

```rust
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ── Data types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub terminal: bool,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    pub count: u32,
    pub last_launch_secs: u64,
}

#[derive(Debug, Default)]
pub struct FrecencyStore {
    pub entries: HashMap<String, FrecencyData>,
}

// ── Public API ───────────────────────────────────────────────────────────────

pub fn scan_apps() -> Vec<AppEntry> { vec![] }

pub fn parse_desktop_file(path: &Path) -> Option<AppEntry> { None }

pub fn strip_exec_codes(exec: &str) -> String { exec.to_string() }

pub fn fuzzy_match(query: &str, text: &str) -> bool { false }

impl FrecencyStore {
    pub fn load() -> Self { Self::default() }
    pub fn record_launch(&mut self, name: &str) {}
    pub fn save(&self) {}
    pub fn score(&self, name: &str) -> f32 { 0.0 }
}

pub fn filtered_apps<'a>(apps: &'a [AppEntry], query: &str, frecency: &FrecencyStore) -> Vec<&'a AppEntry> {
    vec![]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_basic_desktop_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("firefox.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Firefox\nExec=firefox %u\nIcon=firefox\nType=Application\n").unwrap();
        let app = parse_desktop_file(&p).unwrap();
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon, "firefox");
        assert!(!app.terminal);
    }

    #[test]
    fn test_nodisplay_skipped() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("hidden.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Hidden\nExec=foo\nIcon=foo\nType=Application\nNoDisplay=true\n").unwrap();
        assert!(parse_desktop_file(&p).is_none());
    }

    #[test]
    fn test_hidden_skipped() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("hidden.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Hidden\nExec=foo\nIcon=foo\nType=Application\nHidden=true\n").unwrap();
        assert!(parse_desktop_file(&p).is_none());
    }

    #[test]
    fn test_terminal_true_parsed() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("vim.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Vim\nExec=vim\nIcon=vim\nType=Application\nTerminal=true\n").unwrap();
        let app = parse_desktop_file(&p).unwrap();
        assert!(app.terminal);
    }

    #[test]
    fn test_strip_exec_codes() {
        assert_eq!(strip_exec_codes("firefox %u"), "firefox");
        assert_eq!(strip_exec_codes("code %F"), "code");
        assert_eq!(strip_exec_codes("env VAR=1 app %f"), "env VAR=1 app");
        assert_eq!(strip_exec_codes("gimp %U %i"), "gimp");
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match("", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_subsequence() {
        assert!(fuzzy_match("fox", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("FIRE", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert!(!fuzzy_match("xyz", "Firefox"));
    }

    #[test]
    fn test_frecency_score_increases_with_launches() {
        let mut store = FrecencyStore::default();
        let before = store.score("Firefox");
        store.record_launch("Firefox");
        let after = store.score("Firefox");
        assert!(after > before);
    }

    #[test]
    fn test_frecency_save_load_roundtrip() {
        let dir = tempdir().unwrap();
        std::env::set_var("HOME", dir.path().to_str().unwrap());
        let mut store = FrecencyStore::default();
        store.record_launch("Firefox");
        store.save();
        let loaded = FrecencyStore::load();
        assert!(loaded.score("Firefox") > 0.0);
    }

    #[test]
    fn test_scan_apps_from_dir() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("firefox.desktop"),
            "[Desktop Entry]\nName=Firefox\nExec=firefox\nIcon=firefox\nType=Application\n").unwrap();
        fs::write(dir.path().join("hidden.desktop"),
            "[Desktop Entry]\nName=Hidden\nExec=x\nIcon=x\nType=Application\nNoDisplay=true\n").unwrap();
        let apps = scan_apps_from_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Firefox");
    }

    #[test]
    fn test_filtered_apps_empty_query_returns_all() {
        let apps = vec![
            AppEntry { name: "Firefox".into(), exec: "firefox".into(), icon: "".into(), terminal: false, comment: "".into() },
            AppEntry { name: "Code".into(), exec: "code".into(), icon: "".into(), terminal: false, comment: "".into() },
        ];
        let store = FrecencyStore::default();
        let result = filtered_apps(&apps, "", &store);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filtered_apps_query_filters() {
        let apps = vec![
            AppEntry { name: "Firefox".into(), exec: "firefox".into(), icon: "".into(), terminal: false, comment: "".into() },
            AppEntry { name: "Code".into(), exec: "code".into(), icon: "".into(), terminal: false, comment: "".into() },
        ];
        let store = FrecencyStore::default();
        let result = filtered_apps(&apps, "fox", &store);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Firefox");
    }
}
```

- [ ] **Step 2: Run failing tests**

```bash
cd /home/ryan/Projects/mycelium
cargo test -p mycelium apps 2>&1 | grep -E "^(test |FAILED|error)"
```

Expected: compile errors (stubs return wrong types) or test failures — that's expected.

- [ ] **Step 3: Implement the full module**

Replace the stubs with real implementations. The full `src/apps.rs`:

```rust
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub terminal: bool,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    pub count: u32,
    pub last_launch_secs: u64,
}

#[derive(Debug, Default)]
pub struct FrecencyStore {
    pub entries: HashMap<String, FrecencyData>,
}

// ── Frecency ─────────────────────────────────────────────────────────────────

fn frecency_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/mycelium/frecency.json")
}

impl FrecencyStore {
    pub fn load() -> Self {
        let path = frecency_path();
        let Ok(data) = std::fs::read(&path) else { return Self::default() };
        let Ok(entries) = serde_json::from_slice::<HashMap<String, FrecencyData>>(&data) else {
            return Self::default()
        };
        Self { entries }
    }

    pub fn record_launch(&mut self, name: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = self.entries.entry(name.to_string()).or_default();
        entry.count += 1;
        entry.last_launch_secs = now;
        self.save();
    }

    pub fn save(&self) {
        let path = frecency_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("json.tmp");
        if let Ok(data) = serde_json::to_vec_pretty(&self.entries) {
            let _ = std::fs::write(&tmp, &data);
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    /// Frecency score: count * decay where decay = 0.9 ^ days_since_last.
    /// Higher = more relevant.
    pub fn score(&self, name: &str) -> f32 {
        let Some(data) = self.entries.get(name) else { return 0.0 };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (now.saturating_sub(data.last_launch_secs)) as f32 / 86400.0;
        data.count as f32 * 0.9_f32.powf(days)
    }
}

// ── Fuzzy match ───────────────────────────────────────────────────────────────

pub fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() { return true; }
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

// ── Desktop file parsing ──────────────────────────────────────────────────────

/// Strip all XDG Exec field codes (%u %U %f %F %i %c %k etc.)
pub fn strip_exec_codes(exec: &str) -> String {
    let parts: Vec<&str> = exec.split_whitespace()
        .filter(|part| !part.starts_with('%'))
        .collect();
    parts.join(" ")
}

/// Parse a single .desktop file. Returns None if the app should be hidden.
pub fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut in_desktop_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut terminal = false;
    let mut comment = String::new();
    let mut app_type = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry { continue; }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name"       => { if name.is_empty() { name = value.to_string(); } }
                "Exec"       => { if exec.is_empty() { exec = value.to_string(); } }
                "Icon"       => { if icon.is_empty() { icon = value.to_string(); } }
                "Comment"    => { if comment.is_empty() { comment = value.to_string(); } }
                "Type"       => { app_type = value.to_string(); }
                "Terminal"   => { terminal = value.eq_ignore_ascii_case("true"); }
                "NoDisplay"  => { if value.eq_ignore_ascii_case("true") { return None; } }
                "Hidden"     => { if value.eq_ignore_ascii_case("true") { return None; } }
                _ => {}
            }
        }
    }

    if app_type != "Application" || name.is_empty() || exec.is_empty() { return None; }

    Some(AppEntry {
        name,
        exec: strip_exec_codes(&exec),
        icon,
        terminal,
        comment,
    })
}

// ── App scanning ─────────────────────────────────────────────────────────────

/// Scan one directory for .desktop files.
pub fn scan_apps_from_dirs(dirs: &[PathBuf]) -> Vec<AppEntry> {
    let mut apps = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                if let Some(app) = parse_desktop_file(&path) {
                    apps.push(app);
                }
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

/// Scan all standard XDG .desktop locations.
pub fn scan_apps() -> Vec<AppEntry> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from(&home).join(".local/share/applications"),
    ];
    scan_apps_from_dirs(&dirs)
}

// ── Filtered + ranked results ─────────────────────────────────────────────────

/// Returns apps matching `query`, sorted by frecency score descending.
pub fn filtered_apps<'a>(apps: &'a [AppEntry], query: &str, frecency: &FrecencyStore) -> Vec<&'a AppEntry> {
    let mut matched: Vec<&'a AppEntry> = apps.iter()
        .filter(|app| fuzzy_match(query, &app.name))
        .collect();
    // Sort: higher frecency score first, then alphabetical
    matched.sort_by(|a, b| {
        frecency.score(&b.name)
            .partial_cmp(&frecency.score(&a.name))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    matched
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_basic_desktop_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("firefox.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Firefox\nExec=firefox %u\nIcon=firefox\nType=Application\n").unwrap();
        let app = parse_desktop_file(&p).unwrap();
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.exec, "firefox");
        assert_eq!(app.icon, "firefox");
        assert!(!app.terminal);
    }

    #[test]
    fn test_nodisplay_skipped() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("hidden.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Hidden\nExec=foo\nIcon=foo\nType=Application\nNoDisplay=true\n").unwrap();
        assert!(parse_desktop_file(&p).is_none());
    }

    #[test]
    fn test_hidden_skipped() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("hidden.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Hidden\nExec=foo\nIcon=foo\nType=Application\nHidden=true\n").unwrap();
        assert!(parse_desktop_file(&p).is_none());
    }

    #[test]
    fn test_terminal_true_parsed() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("vim.desktop");
        fs::write(&p, "[Desktop Entry]\nName=Vim\nExec=vim\nIcon=vim\nType=Application\nTerminal=true\n").unwrap();
        let app = parse_desktop_file(&p).unwrap();
        assert!(app.terminal);
    }

    #[test]
    fn test_strip_exec_codes() {
        assert_eq!(strip_exec_codes("firefox %u"), "firefox");
        assert_eq!(strip_exec_codes("code %F"), "code");
        assert_eq!(strip_exec_codes("env VAR=1 app %f"), "env VAR=1 app");
        assert_eq!(strip_exec_codes("gimp %U %i"), "gimp");
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match("", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_subsequence() {
        assert!(fuzzy_match("fox", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("FIRE", "Firefox"));
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert!(!fuzzy_match("xyz", "Firefox"));
    }

    #[test]
    fn test_frecency_score_increases_with_launches() {
        let mut store = FrecencyStore::default();
        let before = store.score("Firefox");
        store.record_launch("Firefox");
        let after = store.score("Firefox");
        assert!(after > before);
    }

    #[test]
    fn test_frecency_save_load_roundtrip() {
        let dir = tempdir().unwrap();
        std::env::set_var("HOME", dir.path().to_str().unwrap());
        let mut store = FrecencyStore::default();
        store.record_launch("Firefox");
        store.save();
        let loaded = FrecencyStore::load();
        assert!(loaded.score("Firefox") > 0.0);
    }

    #[test]
    fn test_scan_apps_from_dir() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("firefox.desktop"),
            "[Desktop Entry]\nName=Firefox\nExec=firefox\nIcon=firefox\nType=Application\n").unwrap();
        fs::write(dir.path().join("hidden.desktop"),
            "[Desktop Entry]\nName=Hidden\nExec=x\nIcon=x\nType=Application\nNoDisplay=true\n").unwrap();
        let apps = scan_apps_from_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Firefox");
    }

    #[test]
    fn test_filtered_apps_empty_query_returns_all() {
        let apps = vec![
            AppEntry { name: "Firefox".into(), exec: "firefox".into(), icon: "".into(), terminal: false, comment: "".into() },
            AppEntry { name: "Code".into(), exec: "code".into(), icon: "".into(), terminal: false, comment: "".into() },
        ];
        let store = FrecencyStore::default();
        let result = filtered_apps(&apps, "", &store);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filtered_apps_query_filters() {
        let apps = vec![
            AppEntry { name: "Firefox".into(), exec: "firefox".into(), icon: "".into(), terminal: false, comment: "".into() },
            AppEntry { name: "Code".into(), exec: "code".into(), icon: "".into(), terminal: false, comment: "".into() },
        ];
        let store = FrecencyStore::default();
        let result = filtered_apps(&apps, "fox", &store);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Firefox");
    }

    #[test]
    fn test_frecency_sort_order() {
        let apps = vec![
            AppEntry { name: "Code".into(), exec: "code".into(), icon: "".into(), terminal: false, comment: "".into() },
            AppEntry { name: "Firefox".into(), exec: "firefox".into(), icon: "".into(), terminal: false, comment: "".into() },
        ];
        let mut store = FrecencyStore::default();
        store.record_launch("Firefox");
        store.record_launch("Firefox");
        let result = filtered_apps(&apps, "", &store);
        assert_eq!(result[0].name, "Firefox");
    }
}
```

- [ ] **Step 4: Add `mod apps;` to main.rs and run tests**

In `src/main.rs`, add at the top:
```rust
mod apps;
```

```bash
cd /home/ryan/Projects/mycelium
cargo test apps 2>&1 | grep -E "(test .* ok|FAILED|error\[)"
```

Expected: all `apps::tests::*` tests pass.

- [ ] **Step 5: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/apps.rs src/main.rs
git commit -m "feat: add apps module — XDG desktop file scanning + frecency"
```

---

## Task 5: Create `src/calculator.rs` — Live Expression Evaluation

**Files:**
- Create: `src/calculator.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/calculator.rs`:

```rust
pub fn is_calc_mode(query: &str) -> bool { false }
pub fn evaluate(expr: &str) -> Option<String> { None }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_calc_mode_equals_prefix() {
        assert!(is_calc_mode("=4+4"));
    }

    #[test]
    fn test_is_calc_mode_empty_not_calc() {
        assert!(!is_calc_mode(""));
    }

    #[test]
    fn test_is_calc_mode_plain_text_not_calc() {
        assert!(!is_calc_mode("firefox"));
    }

    #[test]
    fn test_evaluate_addition() {
        assert_eq!(evaluate("4+4"), Some("8".to_string()));
    }

    #[test]
    fn test_evaluate_float() {
        // 1/3 should not be shown as integer
        let result = evaluate("1/3").unwrap();
        assert!(result.contains('.'), "Expected decimal in '{}'", result);
    }

    #[test]
    fn test_evaluate_power() {
        assert_eq!(evaluate("2^10"), Some("1024".to_string()));
    }

    #[test]
    fn test_evaluate_sin() {
        let result = evaluate("sin(0)").unwrap();
        assert_eq!(result, "0");
    }

    #[test]
    fn test_evaluate_sqrt() {
        assert_eq!(evaluate("sqrt(16)"), Some("4".to_string()));
    }

    #[test]
    fn test_evaluate_invalid_returns_none() {
        assert!(evaluate("not an expression!!").is_none());
    }

    #[test]
    fn test_evaluate_incomplete_returns_none() {
        assert!(evaluate("4+").is_none());
    }

    #[test]
    fn test_evaluate_only_equals_returns_none() {
        assert!(evaluate("").is_none());
    }
}
```

- [ ] **Step 2: Run to confirm failures**

```bash
cd /home/ryan/Projects/mycelium
cargo test calculator 2>&1 | grep -E "(FAILED|ok)" | head -20
```

Expected: multiple FAILED (stubs return wrong values).

- [ ] **Step 3: Implement**

Replace `src/calculator.rs` with the full implementation:

```rust
/// Returns true if the query is in calculator mode (starts with '=').
pub fn is_calc_mode(query: &str) -> bool {
    query.starts_with('=')
}

/// Evaluate the expression (with leading '=' already stripped by caller).
/// Returns a formatted string result, or None if evaluation fails.
pub fn evaluate(expr: &str) -> Option<String> {
    if expr.is_empty() { return None; }
    let result = evalexpr::eval(expr).ok()?;
    Some(format_value(result))
}

fn format_value(value: evalexpr::Value) -> String {
    match value {
        evalexpr::Value::Float(f) => {
            // Show as integer if it is exactly an integer value
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", f as i64)
            } else {
                // 6 significant figures
                format!("{:.6}", f)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            }
        }
        evalexpr::Value::Int(i) => format!("{}", i),
        evalexpr::Value::Boolean(b) => format!("{}", b),
        evalexpr::Value::String(s) => s,
        _ => return String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_calc_mode_equals_prefix() {
        assert!(is_calc_mode("=4+4"));
    }

    #[test]
    fn test_is_calc_mode_empty_not_calc() {
        assert!(!is_calc_mode(""));
    }

    #[test]
    fn test_is_calc_mode_plain_text_not_calc() {
        assert!(!is_calc_mode("firefox"));
    }

    #[test]
    fn test_evaluate_addition() {
        assert_eq!(evaluate("4+4"), Some("8".to_string()));
    }

    #[test]
    fn test_evaluate_float() {
        let result = evaluate("1/3").unwrap();
        assert!(result.contains('.'), "Expected decimal in '{}'", result);
    }

    #[test]
    fn test_evaluate_power() {
        assert_eq!(evaluate("2^10"), Some("1024".to_string()));
    }

    #[test]
    fn test_evaluate_sin() {
        let result = evaluate("sin(0)").unwrap();
        assert_eq!(result, "0");
    }

    #[test]
    fn test_evaluate_sqrt() {
        assert_eq!(evaluate("sqrt(16)"), Some("4".to_string()));
    }

    #[test]
    fn test_evaluate_invalid_returns_none() {
        assert!(evaluate("not an expression!!").is_none());
    }

    #[test]
    fn test_evaluate_incomplete_returns_none() {
        assert!(evaluate("4+").is_none());
    }

    #[test]
    fn test_evaluate_only_equals_returns_none() {
        assert!(evaluate("").is_none());
    }
}
```

**Note on evalexpr:** `evalexpr::eval` supports `+`, `-`, `*`, `/`, `^` (power), `sin()`, `cos()`, `tan()`, `sqrt()`, `log()`, `pi`, `e`, etc. Verify the exact function names with `cargo doc --open` if a test fails — evalexpr may use `math::sin` instead of `sin` depending on version.

- [ ] **Step 4: Add mod and run tests**

In `src/main.rs`, add:
```rust
mod calculator;
```

```bash
cd /home/ryan/Projects/mycelium
cargo test calculator 2>&1 | grep -E "(test .* ok|FAILED|error\[)"
```

Expected: all `calculator::tests::*` pass.

- [ ] **Step 5: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/calculator.rs src/main.rs
git commit -m "feat: add calculator module — live evalexpr scientific evaluation"
```

---

## Task 6: Create `src/launcher.rs` — App Spawning

**Files:**
- Create: `src/launcher.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/launcher.rs`:

```rust
use crate::apps::AppEntry;

pub fn launch(entry: &AppEntry) {}

pub fn parse_exec(exec: &str) -> (String, Vec<String>) { (String::new(), vec![]) }

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(exec: &str, terminal: bool) -> AppEntry {
        crate::apps::AppEntry {
            name: "Test".into(),
            exec: exec.into(),
            icon: "".into(),
            terminal,
            comment: "".into(),
        }
    }

    #[test]
    fn test_parse_exec_binary_only() {
        let (bin, args) = parse_exec("firefox");
        assert_eq!(bin, "firefox");
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_exec_with_args() {
        let (bin, args) = parse_exec("code --new-window");
        assert_eq!(bin, "code");
        assert_eq!(args, vec!["--new-window"]);
    }

    #[test]
    fn test_parse_exec_empty() {
        let (bin, args) = parse_exec("");
        assert_eq!(bin, "");
        assert!(args.is_empty());
    }
}
```

- [ ] **Step 2: Run to confirm failures**

```bash
cd /home/ryan/Projects/mycelium
cargo test launcher 2>&1 | grep -E "(FAILED|ok)"
```

- [ ] **Step 3: Implement**

Replace `src/launcher.rs`:

```rust
use crate::apps::AppEntry;

/// The terminal emulator used to wrap Terminal=true apps.
const TERMINAL: &str = "foot";

pub fn launch(entry: &AppEntry) {
    let (bin, args) = parse_exec(&entry.exec);
    if bin.is_empty() { return; }

    let mut cmd = std::process::Command::new(if entry.terminal { TERMINAL } else { &bin });

    if entry.terminal {
        cmd.args(["-e", &bin]);
        cmd.args(&args);
    } else {
        cmd.args(&args);
    }

    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let _ = cmd.spawn();
}

pub fn parse_exec(exec: &str) -> (String, Vec<String>) {
    let mut parts = exec.split_whitespace();
    let bin = parts.next().unwrap_or("").to_string();
    let args = parts.map(str::to_string).collect();
    (bin, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exec_binary_only() {
        let (bin, args) = parse_exec("firefox");
        assert_eq!(bin, "firefox");
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_exec_with_args() {
        let (bin, args) = parse_exec("code --new-window");
        assert_eq!(bin, "code");
        assert_eq!(args, vec!["--new-window"]);
    }

    #[test]
    fn test_parse_exec_empty() {
        let (bin, args) = parse_exec("");
        assert_eq!(bin, "");
        assert!(args.is_empty());
    }
}
```

- [ ] **Step 4: Add mod and run tests**

In `src/main.rs`, add:
```rust
mod launcher;
```

```bash
cd /home/ryan/Projects/mycelium
cargo test launcher 2>&1 | grep -E "(test .* ok|FAILED|error\[)"
```

Expected: all `launcher::tests::*` pass.

- [ ] **Step 5: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/launcher.rs src/main.rs
git commit -m "feat: add launcher module — XDG Exec= parsing + app spawning"
```

---

## Task 7: Rewrite `src/icons.rs` — System Icon Theme Resolution

**Files:**
- Modify: `src/icons.rs`

The rasterization helpers (`rasterize_svg`, `load_image_file`, `load_ico_closest_to`) are reused as-is. The `IconResolver` struct is preserved. Only the `detect_icon_kind` / resolution logic is replaced to use `linicon` for icon name lookup.

- [ ] **Step 1: Replace the full file**

```rust
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub fn rasterize_svg(svg_bytes: &[u8], size: u32) -> Option<(Vec<u8>, u32, u32)> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some((pixmap.data().to_vec(), size, size))
}

pub fn load_image_file(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if ext == "svg" {
        let bytes = std::fs::read(path).ok()?;
        return rasterize_svg(&bytes, 32);
    }
    if ext == "ico" {
        return load_ico_closest_to(path, 32);
    }
    if ext == "xpm" {
        return None; // XPM is rare; skip rather than add a dependency
    }
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

fn load_ico_closest_to(path: &std::path::Path, target: u32) -> Option<(Vec<u8>, u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let dir = ico::IconDir::read(std::io::BufReader::new(file)).ok()?;
    let entry = dir.entries().iter()
        .min_by_key(|e| (e.width() as i32 - target as i32).unsigned_abs())?;
    let image = entry.decode().ok()?;
    let w = image.width();
    let h = image.height();
    Some((image.rgba_data().to_vec(), w, h))
}

/// Resolve an icon name (e.g. "firefox") or absolute path to a rasterized RGBA image.
pub fn resolve_icon_name(icon: &str) -> Option<(Vec<u8>, u32, u32)> {
    // Absolute path: load directly
    if icon.starts_with('/') {
        let p = std::path::Path::new(icon);
        if p.exists() {
            return load_image_file(p);
        }
        return None;
    }

    // Resolve via linicon (searches the active GTK icon theme)
    let icon_path = linicon::lookup_icon(icon)
        .next()
        .map(|ic| ic.path);

    if let Some(path) = icon_path {
        return load_image_file(&path);
    }

    // Fallback: search standard icon dirs manually
    find_icon_in_xdg_dirs(icon)
}

fn find_icon_in_xdg_dirs(name: &str) -> Option<(Vec<u8>, u32, u32)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let search_dirs = [
        format!("{}/.local/share/icons", home),
        "/usr/share/icons".to_string(),
        "/usr/share/pixmaps".to_string(),
    ];
    let sizes = ["256x256", "128x128", "64x64", "48x48", "32x32", "scalable"];
    let categories = ["apps", "applications"];
    let extensions = ["png", "svg", "xpm"];

    for dir in &search_dirs {
        let base = std::path::Path::new(dir);
        // Flat pixmaps structure
        for ext in &extensions {
            let p = base.join(format!("{}.{}", name, ext));
            if p.exists() {
                if let Some(result) = load_image_file(&p) {
                    return Some(result);
                }
            }
        }
        // Hierarchical theme structure (hicolor is the universal fallback theme)
        for size in &sizes {
            for cat in &categories {
                for ext in &extensions {
                    let p = base.join("hicolor").join(size).join(cat).join(format!("{}.{}", name, ext));
                    if p.exists() {
                        if let Some(result) = load_image_file(&p) {
                            return Some(result);
                        }
                    }
                }
            }
        }
    }
    None
}

pub struct IconLoadResult {
    pub key: String,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct IconResolver {
    pending: std::collections::HashSet<String>,
    tx: Sender<IconLoadResult>,
    rx: Receiver<IconLoadResult>,
}

impl IconResolver {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { pending: Default::default(), tx, rx }
    }

    /// Queue an icon name for async resolution.
    pub fn request(&mut self, icon_name: &str) {
        if self.pending.contains(icon_name) { return; }
        self.pending.insert(icon_name.to_string());
        let key = icon_name.to_string();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Some((rgba, width, height)) = resolve_icon_name(&key) {
                let _ = tx.send(IconLoadResult { key, rgba, width, height });
            }
        });
    }

    pub fn poll(&mut self) -> Option<IconLoadResult> {
        self.rx.try_recv().ok()
    }
}

impl Default for IconResolver {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterize_svg_produces_correct_dimensions() {
        // Minimal valid SVG
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"/>"#;
        let (bytes, w, h) = rasterize_svg(svg, 32).unwrap();
        assert_eq!(w, 32);
        assert_eq!(h, 32);
        assert_eq!(bytes.len() as u32, w * h * 4);
    }

    #[test]
    fn test_load_image_file_png() {
        let (bytes, w, h) = load_image_file(std::path::Path::new("tests/fixtures/1x1.png")).unwrap();
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(bytes.len(), 4);
    }

    #[test]
    fn test_resolve_absolute_path() {
        // Should load the PNG when given an absolute path that exists
        let path = std::fs::canonicalize("tests/fixtures/1x1.png").unwrap();
        let result = resolve_icon_name(path.to_str().unwrap());
        assert!(result.is_some());
    }

    #[test]
    fn test_resolve_nonexistent_returns_none() {
        let result = resolve_icon_name("/nonexistent/icon.png");
        assert!(result.is_none());
    }
}
```

**Note:** `linicon::lookup_icon(name)` returns an iterator. Each item has a `.path: PathBuf` field. Run `cargo doc -p linicon --open` to verify the exact API if this doesn't compile. If `linicon` is unavailable or its API differs, the `find_icon_in_xdg_dirs` fallback handles icon resolution without it — you can temporarily comment out the linicon block.

- [ ] **Step 2: Remove the `mod assets;` reference from main.rs and the assets import in icons.rs**

In `src/main.rs`, remove the line:
```rust
mod assets;
```

Delete these source files (assets are no longer needed):
```bash
cd /home/ryan/Projects/mycelium
rm src/assets.rs
rm -rf src/assets/
```

- [ ] **Step 3: Run tests**

```bash
cd /home/ryan/Projects/mycelium
cargo test icons 2>&1 | grep -E "(test .* ok|FAILED|error\[)"
```

Expected: 4 `icons::tests::*` pass.

- [ ] **Step 4: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/icons.rs src/main.rs
git rm src/assets.rs
git rm -r src/assets/
git commit -m "feat: rewrite icons — linicon system theme resolution, drop bundled SVGs"
```

---

## Task 8: Rewrite `src/app.rs` — App State Machine

**Files:**
- Modify: `src/app.rs`

This is the largest change. The `App` struct replaces project-centric state with `Vec<AppEntry>`, `FrecencyStore`, and `calc_result`. The `Mode::Add` is removed. Calculator mode is triggered when the query starts with `=`.

- [ ] **Step 1: Replace the full file**

```rust
use std::collections::HashMap;
use egui::TextureHandle;
use crate::apps::{AppEntry, FrecencyStore, filtered_apps};
use crate::icons::IconResolver;

#[derive(Debug)]
pub enum AppAction {
    Hide,
    LaunchApp(usize),
    CopyToClipboard(String),
}

const ANIM_DURATION: f32 = 0.08;

pub struct App {
    pub query: String,
    pub selected_idx: Option<usize>,
    pub apps: Vec<AppEntry>,
    pub frecency: FrecencyStore,
    pub icon_resolver: IconResolver,
    pub icon_textures: HashMap<String, TextureHandle>,
    pub calc_result: Option<String>,
    pub pending_actions: Vec<AppAction>,
    pub focus_search: bool,
    pub anim_progress: f32,
    pub anim_showing: bool,
    pub anim_hide_pending: bool,
}

impl App {
    pub fn new() -> Self {
        let apps = crate::apps::scan_apps();
        let frecency = FrecencyStore::load();
        App {
            query: String::new(),
            selected_idx: None,
            apps,
            frecency,
            icon_resolver: IconResolver::new(),
            icon_textures: HashMap::new(),
            calc_result: None,
            pending_actions: Vec::new(),
            focus_search: false,
            anim_progress: 0.0,
            anim_showing: false,
            anim_hide_pending: false,
        }
    }

    pub fn on_show(&mut self) {
        self.query.clear();
        self.selected_idx = None;
        self.calc_result = None;
        self.focus_search = true;
        self.anim_showing = true;
        self.anim_progress = 0.0;
        self.anim_hide_pending = false;
    }

    pub fn begin_hide(&mut self) {
        self.anim_showing = false;
    }

    pub fn on_hide(&mut self) {
        self.anim_progress = 0.0;
    }

    pub fn drain_actions(&mut self) -> Vec<AppAction> {
        std::mem::take(&mut self.pending_actions)
    }

    pub fn reload_apps(&mut self) {
        self.apps = crate::apps::scan_apps();
        self.icon_textures.clear();
        self.icon_resolver = IconResolver::new();
    }

    pub fn poll_icons(&mut self, ctx: &egui::Context) {
        while let Some(result) = self.icon_resolver.poll() {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.rgba,
            );
            let handle = ctx.load_texture(
                &result.key,
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.icon_textures.insert(result.key, handle);
        }
    }

    pub fn filtered(&self) -> Vec<&AppEntry> {
        if crate::calculator::is_calc_mode(&self.query) {
            return vec![];
        }
        filtered_apps(&self.apps, &self.query, &self.frecency)
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.poll_icons(ctx);

        // Collect icon names that need loading (block releases borrows before icon_resolver.request)
        let icons_to_load: Vec<String> = {
            let f = filtered_apps(&self.apps, &self.query, &self.frecency);
            f.iter()
                .filter(|app| !app.icon.is_empty() && !self.icon_textures.contains_key(&app.icon))
                .map(|app| app.icon.clone())
                .collect()
        };
        for icon_name in icons_to_load {
            self.icon_resolver.request(&icon_name);
        }

        // Drive animation
        let dt = ctx.input(|i| i.predicted_dt);
        if self.anim_showing {
            self.anim_progress = (self.anim_progress + dt / ANIM_DURATION).min(1.0);
        } else {
            self.anim_progress = (self.anim_progress - dt / ANIM_DURATION).max(0.0);
            if self.anim_progress <= 0.0 {
                self.anim_hide_pending = true;
            }
        }
        let animating = if self.anim_showing { self.anim_progress < 1.0 } else { self.anim_progress > 0.0 };
        if animating { ctx.request_repaint(); }

        let t = ease_out_cubic(self.anim_progress);

        if self.anim_showing {
            let escape_pressed = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if escape_pressed {
                self.begin_hide();
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                ui.set_opacity(t);

                let full_rect = ui.max_rect();
                let rounding = 16.0;
                if full_rect.width() > 0.0 && full_rect.height() > 0.0 {
                    ui.painter().rect_filled(full_rect, rounding, crate::ui::theme::CARD_BG);
                    ui.painter().rect_stroke(full_rect.shrink(1.0), rounding - 1.0,
                        egui::Stroke::new(1.0, crate::ui::theme::BORDER));

                    let builder = egui::UiBuilder::new()
                        .max_rect(full_rect.shrink(1.0))
                        .layout(egui::Layout::top_down(egui::Align::Min));
                    ui.allocate_new_ui(builder, |ui| {
                        ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;
                        ui.style_mut().visuals.selection.bg_fill = crate::ui::theme::ACCENT;
                        ui.style_mut().visuals.widgets.noninteractive.bg_stroke.color =
                            crate::ui::theme::SEPARATOR;

                        let is_calc = crate::calculator::is_calc_mode(&self.query);
                        let placeholder = if is_calc { "= expression..." } else { "Search apps..." };
                        let should_focus = self.focus_search || self.selected_idx.is_none();
                        self.focus_search = false;
                        let search_changed = crate::ui::search::search_bar(ui, &mut self.query, placeholder, should_focus);
                        if search_changed {
                            self.selected_idx = None;
                            // Update calculator result live
                            if is_calc || crate::calculator::is_calc_mode(&self.query) {
                                let expr = self.query.trim_start_matches('=');
                                self.calc_result = crate::calculator::evaluate(expr);
                            } else {
                                self.calc_result = None;
                            }
                        }

                        ui.add(egui::Separator::default().horizontal().spacing(0.0));

                        let hints_height = 40.0f32;
                        let scroll_height = (ui.available_height() - hints_height).max(0.0);

                        egui::ScrollArea::vertical()
                            .max_height(scroll_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.render_list(ui);
                            });

                        crate::ui::hints::hints_bar(ui);
                    });
                }
            });

        if self.anim_showing {
            self.handle_keyboard(ctx);
        }
    }

    fn render_list(&mut self, ui: &mut egui::Ui) {
        // Calculator result row (shown when query starts with '=')
        if crate::calculator::is_calc_mode(&self.query) {
            let expr = self.query.trim_start_matches('=').to_string();
            let result = self.calc_result.clone().unwrap_or_default();
            if !result.is_empty() {
                if crate::ui::list::calc_result_row(ui, &expr, &result, true) {
                    self.copy_calc_result();
                }
            }
            return;
        }

        // Collect (app_idx, selected) pairs up front so we release borrows on
        // self.apps before calling &mut self methods after the loop.
        let rows: Vec<(usize, bool)> = {
            let f = filtered_apps(&self.apps, &self.query, &self.frecency);
            f.iter().enumerate().map(|(row_idx, app)| {
                let app_idx = self.apps.iter().position(|a| std::ptr::eq(a, *app)).unwrap_or(0);
                (app_idx, self.selected_idx == Some(row_idx))
            }).collect()
            // f dropped here — all immutable borrows released
        };

        // Render rows; launch is deferred until after the loop so we don't call
        // &mut self while app/icon still borrow self immutably.
        let mut launch_idx: Option<usize> = None;
        for &(app_idx, selected) in &rows {
            let app = &self.apps[app_idx];
            let icon = self.icon_textures.get(&app.icon);
            if crate::ui::list::app_row(ui, &app.name, &app.comment, icon, selected) {
                launch_idx = Some(app_idx);
            }
            // app and icon borrows end here (end of loop body)
        }
        if let Some(idx) = launch_idx {
            self.launch_app(idx);
        }
    }

    fn launch_app(&mut self, app_idx: usize) {
        if self.pending_actions.iter().any(|a| matches!(a, AppAction::LaunchApp(_))) {
            return;
        }
        let name = self.apps[app_idx].name.clone();
        self.frecency.record_launch(&name);
        self.pending_actions.push(AppAction::LaunchApp(app_idx));
        self.pending_actions.push(AppAction::Hide);
    }

    fn copy_calc_result(&mut self) {
        if let Some(result) = &self.calc_result {
            self.pending_actions.push(AppAction::CopyToClipboard(result.clone()));
        }
        self.pending_actions.push(AppAction::Hide);
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let count = self.selectable_count();
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                    match key {
                        egui::Key::Enter => self.activate_selected(),
                        egui::Key::ArrowDown | egui::Key::Tab => {
                            if modifiers.shift && *key == egui::Key::Tab {
                                self.move_selection(-1, count);
                            } else {
                                self.move_selection(1, count);
                            }
                        }
                        egui::Key::ArrowUp => self.move_selection(-1, count),
                        _ => {}
                    }
                }
            }
        });
    }

    fn selectable_count(&self) -> usize {
        if crate::calculator::is_calc_mode(&self.query) {
            if self.calc_result.is_some() { 1 } else { 0 }
        } else {
            self.filtered().len()
        }
    }

    fn move_selection(&mut self, delta: i32, count: usize) {
        if count == 0 { return; }
        self.selected_idx = match self.selected_idx {
            None if delta > 0 => Some(0),
            None => None,
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 { None } else { Some((next as usize).min(count - 1)) }
            }
        };
    }

    fn activate_selected(&mut self) {
        if crate::calculator::is_calc_mode(&self.query) {
            self.copy_calc_result();
            return;
        }
        let idx = self.selected_idx.unwrap_or(0);
        // Resolve the app index in a block to release the filtered_apps borrow
        // before calling launch_app (which needs &mut self).
        let app_idx: Option<usize> = {
            let f = filtered_apps(&self.apps, &self.query, &self.frecency);
            f.get(idx).map(|app| {
                self.apps.iter().position(|a| std::ptr::eq(a, *app)).unwrap_or(0)
            })
        };
        if let Some(app_idx) = app_idx {
            self.launch_app(app_idx);
        }
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
```

- [ ] **Step 2: Update daemon.rs to handle the new AppActions**

In `src/daemon.rs`, find the actions loop in `render()` (around line 350) and replace it:

```rust
let actions = state.app.drain_actions();
for action in actions {
    match action {
        crate::app::AppAction::Hide => {
            state.app.begin_hide();
            state.request_redraw();
        }
        crate::app::AppAction::LaunchApp(idx) => {
            if let Some(app) = state.app.apps.get(idx) {
                crate::launcher::launch(app);
            }
        }
        crate::app::AppAction::CopyToClipboard(text) => {
            use wl_clipboard_rs::copy::{MimeType, Options, Source};
            let _ = Options::new().copy(
                Source::Bytes(text.into_bytes().into()),
                MimeType::Text,
            );
        }
    }
}
```

Also add at the top of `daemon.rs` where other SIGHUP logic would go — add a `reload_pending` flag for SIGHUP. First, add the field to `State`:

Find `struct State {` and add the field:
```rust
struct State {
    gpu: GpuState,
    win: Option<WindowState>,
    egui_ctx: egui::Context,
    app: crate::app::App,
    reload_pending: bool,
}
```

Initialize it in `init_full` where `State` is constructed:
```rust
self.state = Some(State {
    gpu: GpuState { instance, adapter, device, queue, egui_renderer, surface_format: format },
    win: Some(WindowState { wgpu_surface, surface_config, window, egui_state }),
    egui_ctx,
    app,
    reload_pending: false,
});
```

Add SIGHUP handling in `new_events` (inside the `StartCause::Init` block, after the socket thread):

```rust
// SIGHUP → reload app list
let proxy_sighup = self.proxy.clone();
std::thread::spawn(move || {
    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::signal(libc::SIGHUP, handle_sighup as libc::sighandler_t);
    }
    loop {
        if SIGHUP_RECEIVED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let _ = proxy_sighup.send_event(UserEvent::Reload);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
});
```

And update the signal handler + atomic at the top of `daemon.rs`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
static SIGHUP_RECEIVED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sighup(_: libc::c_int) {
    SIGHUP_RECEIVED.store(true, Ordering::Relaxed);
}
```

Add `libc = "0.2"` to `Cargo.toml` dependencies.

Update `UserEvent`:
```rust
#[derive(Debug)]
enum UserEvent {
    Toggle,
    Reload,
}
```

Handle the new event in `user_event`:
```rust
fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
    match event {
        UserEvent::Toggle => self.toggle(event_loop),
        UserEvent::Reload => {
            if let Some(state) = self.state.as_mut() {
                state.app.reload_apps();
            }
        }
    }
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd /home/ryan/Projects/mycelium
cargo check 2>&1 | grep -E "^error"
```

Expected: no errors (warnings OK). Fix any type mismatches before proceeding.

- [ ] **Step 4: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/app.rs src/daemon.rs Cargo.toml
git commit -m "feat: rewrite app state — AppEntry list, frecency, calculator result, SIGHUP reload"
```

---

## Task 9: Rewrite `src/ui/list.rs` — App Row + Calc Result Row

**Files:**
- Modify: `src/ui/list.rs`

- [ ] **Step 1: Replace the full file**

```rust
use egui::{Ui, Response, Color32, Vec2};
use crate::ui::theme;

fn row_background(ui: &mut Ui, height: f32, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    let bg = if selected {
        theme::ROW_SELECTED
    } else if response.hovered() {
        theme::ROW_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    response
}

/// Renders one application row. Returns true if clicked.
/// Takes name/comment as `&str` so callers don't need to hold `&AppEntry` across the call.
pub fn app_row(
    ui: &mut Ui,
    name: &str,
    comment: &str,
    icon: Option<&egui::TextureHandle>,
    selected: bool,
) -> bool {
    let response = row_background(ui, theme::ROW_H_PROJECT, selected);

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(response.rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(16.0);

    if let Some(tex) = icon {
        child.image(egui::load::SizedTexture::new(tex.id(), [theme::ICON_SIZE, theme::ICON_SIZE]));
    } else {
        child.label(
            egui::RichText::new(egui_phosphor::regular::SQUARES_FOUR)
                .size(theme::ICON_SIZE)
                .color(theme::TEXT_MUTED),
        );
    }
    child.add_space(12.0);

    child.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.style_mut().spacing.item_spacing.y = 1.0;
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(name).size(theme::FONT_TITLE).strong().color(theme::TEXT_PRIMARY),
        );
        if !comment.is_empty() {
            ui.label(
                egui::RichText::new(comment).size(theme::FONT_SUBTITLE).color(theme::TEXT_MUTED),
            );
        }
    });

    response.clicked()
}

/// Renders the calculator result row. Returns true if clicked (to copy result).
pub fn calc_result_row(ui: &mut Ui, expr: &str, result: &str, selected: bool) -> bool {
    if result.is_empty() { return false; }

    let response = row_background(ui, theme::ROW_H_PROJECT, selected);

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(response.rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add_space(16.0);
    child.label(
        egui::RichText::new(egui_phosphor::regular::CALCULATOR)
            .size(theme::ICON_SIZE)
            .color(theme::ACCENT),
    );
    child.add_space(12.0);
    child.label(
        egui::RichText::new(format!("= {}", result)).size(theme::FONT_TITLE).strong().color(theme::ACCENT),
    );

    response.clicked()
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd /home/ryan/Projects/mycelium
cargo check 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/ui/list.rs
git commit -m "feat: rewrite list UI — app_row and calc_result_row"
```

---

## Task 10: Update `src/ui/hints.rs` — Launcher Hints

**Files:**
- Modify: `src/ui/hints.rs`

- [ ] **Step 1: Replace the hints bar content**

Find the `hints_bar` function body and replace it:

```rust
pub fn hints_bar(ui: &mut Ui) {
    ui.add(egui::Separator::default().horizontal().spacing(0.0));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        kbd_icon(ui, egui_phosphor::regular::ARROW_UP);
        kbd_icon(ui, egui_phosphor::regular::ARROW_DOWN);
        hint(ui, "Navigate");
        ui.add_space(16.0);
        kbd_icon(ui, egui_phosphor::regular::KEY_RETURN);
        hint(ui, "Launch / Copy");
        ui.add_space(16.0);
        kbd(ui, "=");
        hint(ui, "Calculator");
        ui.add_space(16.0);
        kbd(ui, "Esc");
        hint(ui, "Close");
    });
    ui.add_space(6.0);
}
```

- [ ] **Step 2: Cargo check**

```bash
cd /home/ryan/Projects/mycelium
cargo check 2>&1 | grep "^error"
```

- [ ] **Step 3: Commit**

```bash
cd /home/ryan/Projects/mycelium
git add src/ui/hints.rs
git commit -m "feat: update hints bar — launcher shortcuts"
```

---

## Task 11: Delete Old Modules + Final Build

**Files:**
- Delete: `src/projects.rs`
- Delete: `src/terminal.rs`
- Delete: `src/paths.rs`
- Modify: `src/main.rs` (remove dead mod declarations)
- Modify: `src/ui/mod.rs` (verify no references to deleted items)

- [ ] **Step 1: Remove dead `mod` declarations from main.rs**

In `src/main.rs`, remove these lines:
```rust
mod assets;      // already removed in Task 7
mod paths;
mod projects;
mod terminal;
```

The final `src/main.rs` should declare only:
```rust
mod app;
mod apps;
mod calculator;
mod daemon;
mod icons;
mod launcher;
mod ui;
```

- [ ] **Step 2: Delete the old source files**

```bash
cd /home/ryan/Projects/mycelium
rm src/projects.rs src/terminal.rs src/paths.rs
```

- [ ] **Step 3: Full release build**

```bash
cd /home/ryan/Projects/mycelium
cargo build --release 2>&1
```

Expected: `Compiling mycelium` → `Finished release [optimized]`. Zero errors.

- [ ] **Step 4: Run all tests**

```bash
cd /home/ryan/Projects/mycelium
cargo test 2>&1 | tail -20
```

Expected: All tests pass. The icons tests for old bundled SVGs are gone; new tests cover apps, calculator, launcher, icons.

- [ ] **Step 5: Commit**

```bash
cd /home/ryan/Projects/mycelium
git rm src/projects.rs src/terminal.rs src/paths.rs
git add src/main.rs
git commit -m "chore: delete replaced modules — projects, terminal, paths"
```

---

## Task 12: Smoke Test + Hyprland Integration Note

- [ ] **Step 1: Start the daemon**

```bash
cd /home/ryan/Projects/mycelium
./target/release/mycelium &
sleep 0.5
```

- [ ] **Step 2: Toggle the launcher**

```bash
./target/release/mycelium --toggle
```

Expected: launcher window appears with a flat list of installed apps.

- [ ] **Step 3: Verify fuzzy search**

Type a few characters (e.g. "fox"). Expected: list filters to apps whose names contain "fox" as a subsequence (e.g. Firefox).

- [ ] **Step 4: Verify calculator mode**

Type `=2^10`. Expected: calculator row appears showing `= 1024`. Press Enter → result copied to clipboard, launcher closes.

- [ ] **Step 5: Verify icon loading**

After the launcher is open for a second, icons should load asynchronously for common apps (Firefox, etc.).

- [ ] **Step 6: Verify frecency**

Launch Firefox. Re-open launcher. Firefox should appear at or near the top of the unfiltered list.

- [ ] **Step 7: Verify SIGHUP reload**

```bash
kill -HUP $(pgrep mycelium)
```

Expected: daemon stays running. Toggle launcher; app list should still work (re-scanned).

- [ ] **Step 8: Add Hyprland keybind (optional)**

In `hyprland.conf`:
```ini
exec-once = mycelium
bind = SUPER, Space, exec, mycelium --toggle
```

---

## Post-Implementation Notes

- **linicon API**: If `linicon::lookup_icon(name).next()` doesn't compile, run `cargo doc -p linicon --open` and adjust. The `find_icon_in_xdg_dirs` fallback in `icons.rs` will catch any apps whose icons linicon can't resolve.
- **evalexpr functions**: If `sin(0)` returns an error, check whether evalexpr uses `math::sin` vs `sin`. Adjust `calculator.rs` if needed.
- **wl-clipboard-rs**: The `Options::new().copy(...)` call requires a Wayland compositor to be running. It will silently fail in headless environments — that's expected.
- **Terminal wrapper**: The `TERMINAL` constant in `launcher.rs` is hardcoded to `foot`. Change it if you use a different terminal for `Terminal=true` apps.
