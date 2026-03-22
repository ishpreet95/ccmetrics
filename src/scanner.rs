use std::path::PathBuf;

use glob::glob;

/// Discovers all JSONL session files under the given base path.
///
/// Scans `{base}/**/*.jsonl` recursively, which includes both main session
/// files and subagent files in `subagents/` subdirectories.
pub fn scan_jsonl_files(base: &std::path::Path) -> anyhow::Result<Vec<PathBuf>> {
    // Escape glob metacharacters in the base path to handle paths with [ ] { }
    let base_str = base.to_string_lossy();
    let escaped = glob::Pattern::escape(&base_str);
    let pattern = format!("{escaped}/**/*.jsonl");
    let mut files: Vec<PathBuf> = glob(&pattern)?
        .filter_map(|entry| entry.ok())
        .filter(|path| path.is_file())
        .collect();

    files.sort();
    Ok(files)
}

/// Determines if a JSONL file is from a subagent based on its path.
///
/// Subagent files live under `.../subagents/agent-*.jsonl`.
pub fn is_subagent_path(path: &std::path::Path) -> bool {
    path.components().any(|c| c.as_os_str() == "subagents")
}

/// Extracts the project path from a JSONL file path.
///
/// Given: `~/.claude/projects/-Users-foo-Desktop-myproject/abc123.jsonl`
/// Returns: `/Users/foo/Desktop/myproject`
///
/// The project directory name uses `-` as a path separator with a leading `-`.
pub fn extract_project_path(file_path: &std::path::Path) -> String {
    // Walk up from the file to find the directory under `projects/`
    let mut path = file_path;
    while let Some(parent) = path.parent() {
        if parent.file_name().map(|n| n == "projects").unwrap_or(false) {
            // `path` is the project directory
            let dirname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            return decode_project_dirname(&dirname);
        }
        path = parent;
    }
    // Fallback: use the file's parent directory name
    file_path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Decodes a Claude Code project directory name to a real path.
///
/// `-Users-foo-Desktop-myproject` → `/Users/foo/Desktop/myproject`
fn decode_project_dirname(dirname: &str) -> String {
    if dirname.starts_with('-') {
        // Replace leading `-` and subsequent `-` with `/`
        let path = dirname.replacen('-', "/", 1);
        path.replace('-', "/")
    } else {
        dirname.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_dirname() {
        // Note: decode is lossy — hyphens in directory names become path separators.
        // e.g., "cc-metrics" decodes as "cc/metrics". This is a known limitation;
        // the parser uses the `cwd` field from JSONL entries for accurate project paths.
        let decoded = decode_project_dirname("-Users-ishpreet-Desktop-personal-cc-metrics");
        assert!(decoded.starts_with("/Users/ishpreet/Desktop/personal/"));
    }

    #[test]
    fn test_decode_simple_dirname() {
        assert_eq!(decode_project_dirname("simple"), "simple");
    }

    #[test]
    fn test_is_subagent_path() {
        let main = PathBuf::from("/home/.claude/projects/foo/abc.jsonl");
        let sub = PathBuf::from("/home/.claude/projects/foo/abc/subagents/agent-123.jsonl");
        assert!(!is_subagent_path(&main));
        assert!(is_subagent_path(&sub));
    }
}
