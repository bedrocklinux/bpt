use crate::*;
use ::function_name::named;
use std::fs::{self, File, FileTimes};
use std::path::Path;
use std::time::{Duration, SystemTime};

#[test]
#[named]
fn sync_dry_run_should_not_write_indexes() {
    setup_test!();

    // Remove pre-synced indexes so dry run starts from a clean state
    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());

    let stdout = run!("sync", "-D", repo_path!("noarch.pkgidx")).unwrap();
    assert!(stdout.contains("Would have:"));
    assert!(stdout.contains("Initialize"));
    assert!(stdout.contains("Dry ran synchronization of 1 index(es)"));

    // Dry run should create the directories but not write index files
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 0);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 0);
}

#[test]
#[named]
fn sync_dry_run_multiple_indexes() {
    setup_test!();

    // Remove pre-synced indexes so dry run starts from a clean state
    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());

    let stdout = run!(
        "sync",
        "-D",
        repo_path!("noarch.pkgidx"),
        repo_path!("noarch.fileidx")
    )
    .unwrap();
    assert!(stdout.contains("Would have:"));
    assert!(stdout.contains("Dry ran synchronization of 2 index(es)"));
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 0);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 0);
}

#[test]
#[named]
fn sync_writes_single_index() {
    setup_test!();

    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());

    let source = repo_path!("noarch.pkgidx");
    let stdout = run!("sync", source).unwrap();
    assert!(stdout.contains("Synchronized"));
    assert!(stdout.contains(source));
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 1);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 0);
}

#[test]
#[named]
fn sync_writes_multiple_indexes() {
    setup_test!();

    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());

    let stdout = run!(
        "sync",
        repo_path!("noarch.pkgidx"),
        repo_path!("noarch.fileidx")
    )
    .unwrap();
    assert!(stdout.contains("Synchronized 2 indexes"));
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 1);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 1);
}

#[test]
#[named]
fn sync_explicit_indexes_do_not_remove_unrelated_current_indexes() {
    setup_test!();

    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());

    run!(
        "sync",
        repo_path!("noarch.pkgidx"),
        repo_path!("noarch.fileidx")
    )
    .unwrap();
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 1);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 1);

    run!("sync", repo_path!("noarch.pkgidx")).unwrap();
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 1);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 1);
}

#[test]
#[named]
fn sync_with_no_configured_indexes_is_noop() {
    setup_test!();

    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());
    std::fs::remove_dir_all(per_test_path!("etc/bpt/repos")).unwrap();
    std::fs::create_dir_all(per_test_path!("etc/bpt/repos")).unwrap();

    let stdout = run!("sync").unwrap();
    assert!(stdout.contains("No indexes configured; nothing to do"));
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/pkgidx")), 0);
    assert_eq!(count_index_files(per_test_path!("var/lib/bpt/fileidx")), 0);
}

#[test]
#[named]
fn sync_rejects_older_target_index_timestamp() {
    setup_test!();

    std::fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());
    let source = repo_path!("noarch.pkgidx");
    run!("sync", source).unwrap();

    // Rebuild a local index later so it has a newer embedded timestamp than the source index.
    std::thread::sleep(Duration::from_secs(1));
    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();
    run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(Path::new(per_test_path!("noarch.pkgidx")).exists());

    let current_path = std::fs::read_dir(per_test_path!("var/lib/bpt/pkgidx"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.file_name().is_some_and(|name| name != ".lock"))
        .unwrap();
    std::fs::copy(per_test_path!("noarch.pkgidx"), &current_path).unwrap();

    let result = run!("sync", "--force", source);
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("older than local instance; possible replay attack"));
}

#[test]
#[named]
fn sync_skips_recent_indexes_unless_forced() {
    setup_test!();

    fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());
    let source = repo_path!("noarch.pkgidx");
    run!("sync", source).unwrap();

    let current_path = synced_idx_path(per_test_path!("var/lib/bpt/pkgidx"));
    set_mtime_recent(&current_path);

    let stdout = run!("sync", source).unwrap();
    assert!(stdout.contains("Synchronized"));
    assert!(stdout.contains("Skipping"));
    assert!(stdout.contains("Still fresh as of "));

    let stdout = run!("sync", "--force", source).unwrap();
    assert!(stdout.contains("Checking"));
}

#[test]
#[named]
fn sync_refreshes_old_indexes() {
    setup_test!();

    fs::remove_dir_all(per_test_path!("var/lib/bpt")).unwrap_or(());
    let source = repo_path!("noarch.pkgidx");
    run!("sync", source).unwrap();

    let current_path = synced_idx_path(per_test_path!("var/lib/bpt/pkgidx"));
    set_mtime_old(&current_path);

    let stdout = run!("sync", source).unwrap();
    assert!(stdout.contains("Checking"));
    assert!(stdout.contains("No update since"));
}

/// Count non-lock files in an index directory.
fn count_index_files(dir: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != ".lock")
        .count()
}

fn synced_idx_path(dir: &str) -> std::path::PathBuf {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.file_name().is_some_and(|name| name != ".lock"))
        .unwrap()
}

fn set_mtime_recent(path: &Path) {
    let file = File::open(path).unwrap();
    let recent_time = SystemTime::now() - Duration::from_secs(30);
    file.set_times(FileTimes::new().set_modified(recent_time))
        .unwrap();
}

fn set_mtime_old(path: &Path) {
    let file = File::open(path).unwrap();
    let old_time = SystemTime::now() - Duration::from_secs(60 * 60 + 5);
    file.set_times(FileTimes::new().set_modified(old_time))
        .unwrap();
}
