use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn runglass_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_runglass"))
}

fn unique_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "runglass-smoke-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("root");
    root
}

#[test]
fn smoke_run_export_bundle_list_snapshot_and_doctor() {
    let root = unique_root("cli");
    let work = root.join("work dir");
    let data = root.join("data");
    fs::create_dir_all(&work).expect("work");

    let run = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["run", "--", "sh", "-c", "printf smoke > 'smoke file.txt'"])
        .output()
        .expect("run command");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let stdout = String::from_utf8_lossy(&run.stdout);
    let run_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Created receipt "))
        .expect("run id")
        .trim()
        .to_string();

    let list = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["list", "--query", "smoke", "--limit", "5"])
        .output()
        .expect("list command");
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert!(String::from_utf8_lossy(&list.stdout).contains(&run_id));

    let snapshot = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["snapshot", "--dry-run"])
        .output()
        .expect("snapshot command");
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    assert!(String::from_utf8_lossy(&snapshot.stdout).contains("Snapshot dry run"));

    let export = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["export", "latest", "--bundle"])
        .output()
        .expect("export command");
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let bundle_path = String::from_utf8_lossy(&export.stdout).trim().to_string();
    let bundle_path = PathBuf::from(bundle_path);
    assert!(bundle_path.exists());
    assert!(bundle_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("runglass-receipt-") && name.ends_with(".tar")));
    let bundle_bytes = fs::read(&bundle_path).expect("bundle bytes");
    for entry in [
        "receipt.html",
        "receipt.md",
        "summary.md",
        "ai-summary.txt",
        "receipt.json",
        "reverse.patch",
        "artifacts/stdout.txt",
        "artifacts/stderr.txt",
        "artifacts/metadata.json",
        "artifacts/file-snapshots",
    ] {
        assert!(
            bundle_bytes
                .windows(entry.len())
                .any(|window| window == entry.as_bytes()),
            "bundle should contain {entry}"
        );
    }

    let ci_out = work.join("ci-receipt");
    let ci_out_arg = ci_out.to_string_lossy().to_string();
    let ci = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args([
            "ci",
            "--provider",
            "generic",
            "--out",
            ci_out_arg.as_str(),
            "--format",
            "html,json,markdown",
            "--",
            "sh",
            "-c",
            "printf ci > ci.txt",
        ])
        .output()
        .expect("ci command");
    assert!(
        ci.status.success(),
        "{}",
        String::from_utf8_lossy(&ci.stderr)
    );
    assert!(ci_out.join("receipt.html").exists());
    assert!(ci_out.join("receipt.json").exists());
    assert!(ci_out.join("receipt.md").exists());
    assert!(ci_out.join("summary.md").exists());
    assert!(ci_out.join("ai-summary.txt").exists());
    assert!(ci_out.join("reverse.patch").exists());
    assert!(ci_out.join("bundle.tar").exists());
    assert!(ci_out.join("artifacts/stdout.txt").exists());
    assert!(ci_out.join("artifacts/stderr.txt").exists());
    assert!(ci_out.join("artifacts/metadata.json").exists());
    assert!(ci_out.join("artifacts/diffs").is_dir());
    assert!(ci_out.join("artifacts/file-snapshots").is_dir());
    assert!(String::from_utf8_lossy(&ci.stdout).contains("Created CI receipt"));
    assert!(String::from_utf8_lossy(&ci.stdout).contains("bundle.tar"));
    assert!(
        String::from_utf8_lossy(&fs::read(ci_out.join("receipt.json")).expect("ci json"))
            .contains("\"ci\"")
    );

    let validate_latest = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["validate", "latest"])
        .output()
        .expect("validate latest command");
    assert!(
        validate_latest.status.success(),
        "{}",
        String::from_utf8_lossy(&validate_latest.stderr)
    );
    assert!(
        String::from_utf8_lossy(&validate_latest.stdout).contains("RunGlass Receipt Validation")
    );

    let validate_ci = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args(["validate", ci_out_arg.as_str()])
        .output()
        .expect("validate ci command");
    assert!(
        validate_ci.status.success(),
        "{}",
        String::from_utf8_lossy(&validate_ci.stderr)
    );
    assert!(String::from_utf8_lossy(&validate_ci.stdout).contains("Validation passed"));

    let ci_receipt_json = ci_out.join("receipt.json").to_string_lossy().to_string();
    let github_dry_run = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .args([
            "github",
            "comment",
            "--receipt",
            ci_receipt_json.as_str(),
            "--repo",
            "error311/runglass",
            "--pr",
            "1",
            "--dry-run",
        ])
        .output()
        .expect("github dry-run command");
    assert!(
        github_dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&github_dry_run.stderr)
    );
    let github_body = String::from_utf8_lossy(&github_dry_run.stdout);
    assert!(github_body.contains("## RunGlass Receipt"));
    assert!(github_body.contains("Full receipt: generated locally by RunGlass."));

    let doctor = Command::new(runglass_bin())
        .env("RUNGLASS_DATA_HOME", &data)
        .current_dir(&work)
        .arg("doctor")
        .output()
        .expect("doctor command");
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("RunGlass Doctor"));
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("Platform"));

    fs::remove_dir_all(root).ok();
}
