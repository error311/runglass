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
    assert!(PathBuf::from(bundle_path).exists());

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

    fs::remove_dir_all(root).ok();
}
