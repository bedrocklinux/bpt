use crate::*;
use ::function_name::named;
use std::io::Write;
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};

static MAKE_REPO_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn make_repo_lock() -> MutexGuard<'static, ()> {
    MAKE_REPO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn count_files_with_ext(dir: &str, ext: &str) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == std::ffi::OsStr::new(ext))
        })
        .count()
}

#[test]
#[named]
fn make_repo_errors_without_bbuilds() {
    let _guard = make_repo_lock();
    setup_test!();

    let result = run_bpt_make_repo!("-O", per_test_path!());
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("No *.bbuild files found"));
    assert!(stderr.contains(per_test_path!("")));
}

#[test]
#[named]
fn make_repo_creates_repo_artifacts() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-song-gen@1.0.0.bbuild"),
        per_test_path!("fakeblock-song-gen@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(stdout.contains("Updated repository files from 3 *.bbuild file(s)"));

    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("fakeblock-songs@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("noarch.pkgidx")).exists());
    assert!(Path::new(per_test_path!("bbuild.pkgidx")).exists());
    assert!(Path::new(per_test_path!("noarch.fileidx")).exists());

    assert_eq!(count_files_with_ext(per_test_path!(""), "bpt"), 3);
    assert_eq!(count_files_with_ext(per_test_path!(""), "pkgidx"), 3);
    assert_eq!(count_files_with_ext(per_test_path!(""), "fileidx"), 2);
}

#[test]
#[named]
fn make_repo_noop_when_already_synchronized() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let _ = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    let stdout = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(stdout.contains("No changes needed"));
}

#[test]
#[named]
fn make_repo_prompt_decline_aborts() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let result = run_bpt_make_repo_prompt!(b"n\n", "-O", per_test_path!());
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Confirmation prompt denied"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn make_repo_prompt_accept_continues() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_make_repo_prompt!(b"y\n", "-O", per_test_path!()).unwrap();
    assert!(stdout.contains("Continuing will:"));
    assert!(stdout.contains("\n\nUpdated repository files from 1 *.bbuild file(s)"));
    assert!(stdout.contains("Updated repository files from 1 *.bbuild file(s)"));
    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn make_repo_removes_stale_bpt_and_updates_indexes() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-song-gen@1.0.0.bbuild"),
        per_test_path!("fakeblock-song-gen@1.0.0.bbuild"),
    )
    .unwrap();
    let _ = run_bpt_make_repo!("-O", per_test_path!()).unwrap();

    std::fs::remove_file(per_test_path!("fakeblock-song-gen@1.0.0.bbuild")).unwrap();

    let stdout = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(stdout.contains("Updated repository files from 2 *.bbuild file(s)"));
    assert!(!Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
    assert_eq!(count_files_with_ext(per_test_path!(""), "bpt"), 2);
}

#[test]
#[named]
fn make_repo_updates_when_bbuild_is_newer() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let _ = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    let first_bpt_mtime = std::fs::metadata(per_test_path!("fakeblock@1.0.0:noarch.bpt"))
        .unwrap()
        .modified()
        .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));
    std::fs::OpenOptions::new()
        .append(true)
        .open(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap()
        .write_all(b"\n# touch for mtime-based update\n")
        .unwrap();

    let stdout = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(stdout.contains("Update"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch.bpt"));

    let second_bpt_mtime = std::fs::metadata(per_test_path!("fakeblock@1.0.0:noarch.bpt"))
        .unwrap()
        .modified()
        .unwrap();
    assert!(second_bpt_mtime > first_bpt_mtime);
}

#[test]
#[named]
fn make_repo_dry_run_should_not_write_artifacts() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_make_repo!("-D", "-O", per_test_path!()).unwrap();
    assert!(stdout.contains("\n\nDry ran make-repo of 1 *.bbuild file(s)"));

    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(!Path::new(per_test_path!("noarch.pkgidx")).exists());
    assert!(!Path::new(per_test_path!("bbuild.pkgidx")).exists());
    assert!(!Path::new(per_test_path!("noarch.fileidx")).exists());
}

#[test]
#[named]
fn make_repo_removes_stale_indexes_for_unconfigured_arch() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    std::fs::copy(
        per_test_path!("noarch.pkgidx"),
        per_test_path!("aarch64.pkgidx"),
    )
    .unwrap();
    std::fs::copy(
        per_test_path!("noarch.fileidx"),
        per_test_path!("aarch64.fileidx"),
    )
    .unwrap();
    assert!(Path::new(per_test_path!("aarch64.pkgidx")).exists());
    assert!(Path::new(per_test_path!("aarch64.fileidx")).exists());

    run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(!Path::new(per_test_path!("aarch64.pkgidx")).exists());
    assert!(!Path::new(per_test_path!("aarch64.fileidx")).exists());
}

#[test]
#[named]
fn make_repo_rejects_invalid_pkgidx_filename_stem() {
    let _guard = make_repo_lock();
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    run_bpt_make_repo!("-O", per_test_path!()).unwrap();

    std::fs::copy(
        per_test_path!("noarch.pkgidx"),
        per_test_path!("not-an-arch.pkgidx"),
    )
    .unwrap();

    let result = run_bpt_make_repo!("-O", per_test_path!());
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Filename stem is not a recognized architecture"));
    assert!(stderr.contains("not-an-arch.pkgidx"));
}

#[test]
#[named]
fn make_repo_orders_builds_by_makedepends() {
    let _guard = make_repo_lock();
    setup_test!();

    std::fs::copy(
        repo_path!("aaa-consumer@1.0.0.bbuild"),
        per_test_path!("aaa-consumer@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("zzz-helper@1.0.0.bbuild"),
        per_test_path!("zzz-helper@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_make_repo!("-O", per_test_path!()).unwrap();
    assert!(stdout.contains("Updated repository files from 2 *.bbuild file(s)"));
    assert!(Path::new(per_test_path!("aaa-consumer@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("zzz-helper@1.0.0:noarch.bpt")).exists());

    let files = run!("files", per_test_path!("aaa-consumer@1.0.0:noarch.bpt")).unwrap();
    assert!(files.contains("usr/share/aaa-consumer/generated.txt"));
}
