//! Repository structure gates (ai-rules.md Phase 2).

use std::fs;
use std::path::Path;

/// Strict 250-line limit per `.rs` file (production and tests).
const MAX_RS_LINES: usize = 250;

#[test]
fn test_rs_file_line_limit_250() {
    fn check_dir(dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name != "target" && name != ".git" {
                        check_dir(&path);
                    }
                } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    let content = fs::read_to_string(&path).expect("failed to read file");
                    let lines = content.lines().count();
                    assert!(
                        lines <= MAX_RS_LINES,
                        "File {:?} has {} lines, exceeding {} limit (ai-rules.md)",
                        path,
                        lines,
                        MAX_RS_LINES
                    );
                }
            }
        }
    }
    check_dir(Path::new("."));
}

#[test]
fn test_cargo_rfc_file_tree_structure() {
    assert!(Path::new("Cargo.toml").exists(), "Cargo.toml must exist");
    assert!(Path::new("src/lib.rs").exists(), "src/lib.rs must exist");
    assert!(Path::new("src/main.rs").exists(), "src/main.rs must exist");
    assert!(
        Path::new("tests/integration_tests.rs").exists(),
        "tests/integration_tests.rs must exist"
    );
    assert!(
        Path::new("ai-rules.md").exists(),
        "ai-rules.md must exist at repo root"
    );
}

#[test]
fn test_only_rust_code_in_src() {
    fn check_src_dir(dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    check_src_dir(&path);
                } else {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    assert!(
                        ext == "rs" || ext == "jpg" || ext == "png",
                        "Non-Rust code file found in src/: {:?}",
                        path
                    );
                }
            }
        }
    }
    check_src_dir(Path::new("src"));
}
