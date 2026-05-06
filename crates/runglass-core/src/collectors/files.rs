use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};

use crate::reporting::risk_tags;
use crate::storage::reports_dir;
use crate::{
    FileChange, FileChangeType, SkippedSnapshotFile, SnapshotDirectoryStats, SnapshotEntry,
    TextDiff,
};

pub const DEFAULT_MAX_SNAPSHOT_FILE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Default)]
struct SnapshotIgnoreMatcher {
    relative_rules: Vec<GlobMatcher>,
    basename_rules: Vec<GlobMatcher>,
    relative_dir_rules: Vec<GlobMatcher>,
    basename_dir_rules: Vec<GlobMatcher>,
}

pub fn snapshot_file_byte_limit() -> u64 {
    env::var("RUNGLASS_MAX_SNAPSHOT_BYTES")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_SNAPSHOT_FILE_BYTES)
}

pub fn snapshot_directory(root: &Path) -> Result<HashMap<String, SnapshotEntry>> {
    Ok(snapshot_directory_with_stats(root)?.0)
}

pub fn snapshot_directory_with_stats(
    root: &Path,
) -> Result<(HashMap<String, SnapshotEntry>, SnapshotDirectoryStats)> {
    let mut entries = HashMap::new();
    let mut stats = SnapshotDirectoryStats::default();
    let reports_root = reports_dir().ok();
    let byte_limit = snapshot_file_byte_limit();
    let ignore_matcher = load_snapshot_ignore_matcher(root)?;
    visit_dir(
        root,
        root,
        &mut entries,
        reports_root.as_deref(),
        &mut stats,
        byte_limit,
        &ignore_matcher,
    )?;
    Ok((entries, stats))
}

pub fn diff_snapshots(
    before: &HashMap<String, SnapshotEntry>,
    after: &HashMap<String, SnapshotEntry>,
) -> Vec<FileChange> {
    let mut paths = BTreeSet::new();
    paths.extend(before.keys().cloned());
    paths.extend(after.keys().cloned());

    let mut changes = Vec::new();
    for path in paths {
        match (before.get(&path), after.get(&path)) {
            (None, Some(after_entry)) => changes.push(FileChange {
                path: path.clone(),
                change_type: FileChangeType::Created,
                before_hash: None,
                after_hash: Some(after_entry.hash.clone()),
                before_size: None,
                after_size: Some(after_entry.size),
                is_text: after_entry.is_text,
                diff: None,
                risk_tags: risk_tags(&path, after_entry.executable),
                before_artifact_path: None,
                after_artifact_path: None,
                before_executable: None,
                after_executable: Some(after_entry.executable),
            }),
            (Some(before_entry), None) => changes.push(FileChange {
                path: path.clone(),
                change_type: FileChangeType::Deleted,
                before_hash: Some(before_entry.hash.clone()),
                after_hash: None,
                before_size: Some(before_entry.size),
                after_size: None,
                is_text: before_entry.is_text,
                diff: None,
                risk_tags: risk_tags(&path, before_entry.executable),
                before_artifact_path: None,
                after_artifact_path: None,
                before_executable: Some(before_entry.executable),
                after_executable: None,
            }),
            (Some(before_entry), Some(after_entry)) if before_entry.hash != after_entry.hash => {
                let diff = match (&before_entry.text, &after_entry.text) {
                    (Some(before_text), Some(after_text)) => Some(TextDiff {
                        format: "unified".to_string(),
                        content: simple_unified_diff(before_text, after_text),
                    }),
                    _ => None,
                };

                changes.push(FileChange {
                    path: path.clone(),
                    change_type: FileChangeType::Modified,
                    before_hash: Some(before_entry.hash.clone()),
                    after_hash: Some(after_entry.hash.clone()),
                    before_size: Some(before_entry.size),
                    after_size: Some(after_entry.size),
                    is_text: before_entry.is_text && after_entry.is_text,
                    diff,
                    risk_tags: risk_tags(&path, after_entry.executable),
                    before_artifact_path: None,
                    after_artifact_path: None,
                    before_executable: Some(before_entry.executable),
                    after_executable: Some(after_entry.executable),
                });
            }
            _ => {}
        }
    }

    changes
}

fn visit_dir(
    root: &Path,
    current: &Path,
    entries: &mut HashMap<String, SnapshotEntry>,
    reports_root: Option<&Path>,
    stats: &mut SnapshotDirectoryStats,
    byte_limit: u64,
    ignore_matcher: &SnapshotIgnoreMatcher,
) -> Result<()> {
    let dir_entries = match fs::read_dir(current) {
        Ok(dir_entries) => dir_entries,
        Err(error) if current == root => {
            return Err(error)
                .with_context(|| format!("failed to read snapshot root {}", root.display()));
        }
        Err(_) => return Ok(()),
    };

    for entry in dir_entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let relative = relative_display(root, &path);

        if should_ignore(&path, &relative, &file_type, reports_root, ignore_matcher) {
            continue;
        }

        if file_type.is_dir() {
            visit_dir(
                root,
                &path,
                entries,
                reports_root,
                stats,
                byte_limit,
                ignore_matcher,
            )?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() > byte_limit {
            stats.skipped_large_files.push(SkippedSnapshotFile {
                path: relative.clone(),
                size: metadata.len(),
            });
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let text = if bytes.len() <= 128 * 1024 {
            String::from_utf8(bytes.clone()).ok()
        } else {
            None
        };
        let is_text = text.is_some();
        let hash = hash_bytes(&bytes);
        let executable = is_executable(&metadata);

        entries.insert(
            relative.clone(),
            SnapshotEntry {
                path: relative,
                hash,
                size: metadata.len(),
                bytes,
                text,
                is_text,
                executable,
            },
        );
    }

    Ok(())
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn should_ignore(
    path: &Path,
    relative: &str,
    file_type: &fs::FileType,
    reports_root: Option<&Path>,
    ignore_matcher: &SnapshotIgnoreMatcher,
) -> bool {
    if reports_root.is_some_and(|root| path.starts_with(root)) {
        return true;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if builtin_ignore(name, file_type) {
        return true;
    }
    ignore_matcher.matches(relative, name, file_type.is_dir())
}

fn builtin_ignore(name: &str, file_type: &fs::FileType) -> bool {
    if file_type.is_dir() {
        return matches!(
            name,
            ".git"
                | "node_modules"
                | "target"
                | "vendor"
                | ".cache"
                | ".npm-cache"
                | ".pnpm-store"
                | "__pycache__"
        );
    }
    name.ends_with(".log") || name.ends_with(".tmp")
}

fn load_snapshot_ignore_matcher(root: &Path) -> Result<SnapshotIgnoreMatcher> {
    let path = root.join(".runglassignore");
    if !path.exists() {
        return Ok(SnapshotIgnoreMatcher::default());
    }

    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut matcher = SnapshotIgnoreMatcher::default();
    for (line_number, raw_line) in raw.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        matcher.add_rule(line).with_context(|| {
            format!(
                "invalid .runglassignore pattern on line {}: {}",
                line_number + 1,
                line
            )
        })?;
    }
    Ok(matcher)
}

impl SnapshotIgnoreMatcher {
    fn add_rule(&mut self, pattern: &str) -> Result<()> {
        let directory_only = pattern.ends_with('/');
        let normalized = pattern.trim_end_matches('/');
        if normalized.is_empty() {
            return Ok(());
        }

        let matcher = build_glob_matcher(normalized)?;
        let has_path_separator = normalized.contains('/');
        match (directory_only, has_path_separator) {
            (true, true) => self.relative_dir_rules.push(matcher),
            (true, false) => self.basename_dir_rules.push(matcher),
            (false, true) => self.relative_rules.push(matcher),
            (false, false) => self.basename_rules.push(matcher),
        }
        Ok(())
    }

    fn matches(&self, relative: &str, basename: &str, is_dir: bool) -> bool {
        if self
            .relative_rules
            .iter()
            .any(|rule| rule.is_match(relative))
        {
            return true;
        }
        if self
            .basename_rules
            .iter()
            .any(|rule| rule.is_match(basename))
        {
            return true;
        }
        if is_dir {
            if self
                .relative_dir_rules
                .iter()
                .any(|rule| rule.is_match(relative))
            {
                return true;
            }
            if self
                .basename_dir_rules
                .iter()
                .any(|rule| rule.is_match(basename))
            {
                return true;
            }
        }
        false
    }
}

fn build_glob_matcher(pattern: &str) -> Result<GlobMatcher> {
    Ok(Glob::new(pattern)?.compile_matcher())
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn is_executable(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

pub(crate) fn simple_unified_diff(before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let max_lines = before_lines.len().max(after_lines.len());
    let mut lines = vec![format!(
        "@@ -1,{} +1,{} @@",
        before_lines.len(),
        after_lines.len()
    )];

    for index in 0..max_lines {
        match (before_lines.get(index), after_lines.get(index)) {
            (Some(left), Some(right)) if left == right => lines.push(format!(" {}", left)),
            (Some(left), Some(right)) => {
                lines.push(format!("-{}", left));
                lines.push(format!("+{}", right));
            }
            (Some(left), None) => lines.push(format!("-{}", left)),
            (None, Some(right)) => lines.push(format!("+{}", right)),
            (None, None) => {}
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{snapshot_directory_with_stats, DEFAULT_MAX_SNAPSHOT_FILE_BYTES};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn snapshot_skips_oversized_files_before_reading_bytes() {
        let _guard = env_lock().lock().expect("lock test env");
        let root = env::temp_dir().join(format!(
            "runglass-large-file-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        env::set_var("RUNGLASS_DATA_HOME", root.join("data-home"));
        env::remove_var("RUNGLASS_MAX_SNAPSHOT_BYTES");

        std::fs::write(root.join("small.txt"), b"small\n").expect("write small");
        std::fs::write(
            root.join("huge.bin"),
            vec![0u8; DEFAULT_MAX_SNAPSHOT_FILE_BYTES as usize + 1],
        )
        .expect("write huge");

        let (entries, stats) = snapshot_directory_with_stats(&root).expect("snapshot");
        assert!(entries.contains_key("small.txt"));
        assert!(!entries.contains_key("huge.bin"));
        assert_eq!(stats.skipped_large_files.len(), 1);
        assert_eq!(stats.skipped_large_files[0].path, "huge.bin");
    }

    #[test]
    fn snapshot_size_cap_can_be_overridden_by_env() {
        let _guard = env_lock().lock().expect("lock test env");
        let root = env::temp_dir().join(format!(
            "runglass-size-cap-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        env::set_var("RUNGLASS_DATA_HOME", root.join("data-home"));
        env::set_var("RUNGLASS_MAX_SNAPSHOT_BYTES", "4");

        std::fs::write(root.join("tiny.txt"), b"12345").expect("write tiny");
        let (entries, stats) = snapshot_directory_with_stats(&root).expect("snapshot");

        assert!(!entries.contains_key("tiny.txt"));
        assert_eq!(stats.skipped_large_files.len(), 1);
        assert_eq!(stats.skipped_large_files[0].path, "tiny.txt");
        env::remove_var("RUNGLASS_MAX_SNAPSHOT_BYTES");
    }

    #[test]
    fn runglassignore_can_skip_relative_and_basename_matches() {
        let _guard = env_lock().lock().expect("lock test env");
        let root = unique_test_root("runglassignore");
        env::set_var("RUNGLASS_DATA_HOME", root.join("data-home"));
        env::remove_var("RUNGLASS_MAX_SNAPSHOT_BYTES");

        std::fs::create_dir_all(root.join("cache-dir")).expect("create cache dir");
        std::fs::create_dir_all(root.join("nested/keep")).expect("create nested keep");
        std::fs::create_dir_all(root.join("nested/tmpdir")).expect("create nested tmpdir");
        std::fs::write(
            root.join(".runglassignore"),
            "cache-dir/\n*.sqlite\nnested/tmpdir/\nsecret.txt\n",
        )
        .expect("write ignore file");
        std::fs::write(root.join("cache-dir/data.txt"), b"ignore me").expect("write cache file");
        std::fs::write(root.join("app.sqlite"), b"db").expect("write sqlite file");
        std::fs::write(root.join("secret.txt"), b"secret").expect("write secret");
        std::fs::write(root.join("nested/tmpdir/file.txt"), b"temp").expect("write temp");
        std::fs::write(root.join("nested/keep/file.txt"), b"keep").expect("write keep");

        let (entries, stats) = snapshot_directory_with_stats(&root).expect("snapshot");
        assert!(stats.skipped_large_files.is_empty());
        assert!(!entries.contains_key("cache-dir/data.txt"));
        assert!(!entries.contains_key("app.sqlite"));
        assert!(!entries.contains_key("secret.txt"));
        assert!(!entries.contains_key("nested/tmpdir/file.txt"));
        assert!(entries.contains_key("nested/keep/file.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_skips_unreadable_child_directories() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = env_lock().lock().expect("lock test env");
        let root = unique_test_root("unreadable-child");
        env::set_var("RUNGLASS_DATA_HOME", root.join("data-home"));
        env::remove_var("RUNGLASS_MAX_SNAPSHOT_BYTES");

        std::fs::write(root.join("visible.txt"), b"visible").expect("write visible file");
        let unreadable = root.join("blocked");
        std::fs::create_dir_all(&unreadable).expect("create unreadable dir");
        std::fs::write(unreadable.join("hidden.txt"), b"hidden").expect("write hidden file");
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
            .expect("block unreadable dir");

        let result = snapshot_directory_with_stats(&root);

        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o755))
            .expect("restore unreadable dir");

        let (entries, _stats) = result.expect("snapshot should skip unreadable child");
        assert!(entries.contains_key("visible.txt"));
        assert!(!entries.contains_key("blocked/hidden.txt"));
    }

    fn unique_test_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "runglass-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale root");
        }
        std::fs::create_dir_all(&root).expect("create root");
        root
    }
}
