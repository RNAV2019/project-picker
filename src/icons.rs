use std::path::Path;
use crate::assets;

#[derive(Debug, Clone, PartialEq)]
pub enum IconKind {
    BundledSvg(&'static [u8]),
    ImageFile(std::path::PathBuf),
    Folder,
}

pub fn detect_icon_kind(project_path: &str) -> IconKind {
    let root = Path::new(project_path);

    // 1. Check for image files first
    let image_candidates = [
        "icon.png", "logo.png", "logo.svg", "favicon.ico",
        ".github/logo.png", ".github/LOGO.png",
    ];
    for name in &image_candidates {
        let candidate = root.join(name);
        if candidate.exists() {
            return IconKind::ImageFile(candidate);
        }
    }

    // 2. Stack detection via marker files
    let markers: &[(&str, &[u8])] = &[
        ("Cargo.toml",       assets::RUST_SVG),
        ("tsconfig.json",    assets::TYPESCRIPT_SVG),
        ("package.json",     assets::JAVASCRIPT_SVG),
        ("go.mod",           assets::GO_SVG),
        ("pyproject.toml",   assets::PYTHON_SVG),
        ("requirements.txt", assets::PYTHON_SVG),
        ("Gemfile",          assets::RUBY_SVG),
        ("pom.xml",          assets::JAVA_SVG),
        ("build.gradle",     assets::JAVA_SVG),
        ("CMakeLists.txt",   assets::CPP_SVG),
    ];
    for (marker, svg) in markers {
        if root.join(marker).exists() {
            return IconKind::BundledSvg(svg);
        }
    }
    // Check for .py files
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("py") {
                return IconKind::BundledSvg(assets::PYTHON_SVG);
            }
        }
    }

    // 3. Fallback
    IconKind::Folder
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detects_rust_project() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), b"[package]").unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::BundledSvg(crate::assets::RUST_SVG));
    }

    #[test]
    fn test_detects_node_project() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), b"{}").unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::BundledSvg(crate::assets::JAVASCRIPT_SVG));
    }

    #[test]
    fn test_image_file_takes_priority() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), b"[package]").unwrap();
        let png_path = dir.path().join("icon.png");
        fs::write(&png_path, include_bytes!("../tests/fixtures/1x1.png")).unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::ImageFile(png_path));
    }

    #[test]
    fn test_fallback_to_folder() {
        let dir = tempdir().unwrap();
        let kind = detect_icon_kind(dir.path().to_str().unwrap());
        assert_eq!(kind, IconKind::Folder);
    }
}
