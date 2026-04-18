use crate::e2e::common::run::run_bpt_at_with_envs;
use crate::*;
use ::function_name::named;
use std::path::Path;

#[test]
#[named]
fn clean_without_flags_removes_package_and_source_cache() {
    setup_test!();

    let cache_home = per_test_path!("cache-home");
    let pkg_cache = format!("{cache_home}/bpt/pkgs");
    let src_cache = format!("{cache_home}/bpt/src");
    std::fs::create_dir_all(&pkg_cache).unwrap();
    std::fs::create_dir_all(&src_cache).unwrap();
    std::fs::write(format!("{pkg_cache}/pkg-entry"), "pkg").unwrap();
    std::fs::write(format!("{src_cache}/src-entry"), "src").unwrap();

    let stdout =
        run_bpt_at_with_envs(per_test_path!(), &["clean"], &[("XDG_CACHE_HOME", cache_home)])
            .unwrap();
    assert!(stdout.contains("Removed 2 cached items"));
    assert!(!Path::new(&format!("{pkg_cache}/pkg-entry")).exists());
    assert!(!Path::new(&format!("{src_cache}/src-entry")).exists());
}

#[test]
#[named]
fn clean_packages_only_preserves_source_cache() {
    setup_test!();

    let cache_home = per_test_path!("cache-home");
    let pkg_cache = format!("{cache_home}/bpt/pkgs");
    let src_cache = format!("{cache_home}/bpt/src");
    std::fs::create_dir_all(&pkg_cache).unwrap();
    std::fs::create_dir_all(&src_cache).unwrap();
    std::fs::write(format!("{pkg_cache}/pkg-entry"), "pkg").unwrap();
    std::fs::write(format!("{src_cache}/src-entry"), "src").unwrap();

    let stdout = run_bpt_at_with_envs(
        per_test_path!(),
        &["clean", "--packages"],
        &[("XDG_CACHE_HOME", cache_home)],
    )
    .unwrap();
    assert!(stdout.contains("Removed 1 cached items"));
    assert!(!Path::new(&format!("{pkg_cache}/pkg-entry")).exists());
    assert!(Path::new(&format!("{src_cache}/src-entry")).exists());
}

#[test]
#[named]
fn clean_source_only_preserves_package_cache() {
    setup_test!();

    let cache_home = per_test_path!("cache-home");
    let pkg_cache = format!("{cache_home}/bpt/pkgs");
    let src_cache = format!("{cache_home}/bpt/src");
    std::fs::create_dir_all(&pkg_cache).unwrap();
    std::fs::create_dir_all(&src_cache).unwrap();
    std::fs::write(format!("{pkg_cache}/pkg-entry"), "pkg").unwrap();
    std::fs::write(format!("{src_cache}/src-entry"), "src").unwrap();

    let stdout = run_bpt_at_with_envs(
        per_test_path!(),
        &["clean", "--source"],
        &[("XDG_CACHE_HOME", cache_home)],
    )
    .unwrap();
    assert!(stdout.contains("Removed 1 cached items"));
    assert!(Path::new(&format!("{pkg_cache}/pkg-entry")).exists());
    assert!(!Path::new(&format!("{src_cache}/src-entry")).exists());
}

#[test]
#[named]
fn clean_dry_run_does_not_remove_cache_entries() {
    setup_test!();

    let cache_home = per_test_path!("cache-home");
    let pkg_cache = format!("{cache_home}/bpt/pkgs");
    let src_cache = format!("{cache_home}/bpt/src");
    std::fs::create_dir_all(&pkg_cache).unwrap();
    std::fs::create_dir_all(&src_cache).unwrap();
    std::fs::write(format!("{pkg_cache}/pkg-entry"), "pkg").unwrap();
    std::fs::write(format!("{src_cache}/src-entry"), "src").unwrap();

    let stdout =
        run_bpt_at_with_envs(per_test_path!(), &["clean", "-D"], &[("XDG_CACHE_HOME", cache_home)])
            .unwrap();
    assert!(stdout.contains("Dry run would have removed 2 cached items"));
    assert!(Path::new(&format!("{pkg_cache}/pkg-entry")).exists());
    assert!(Path::new(&format!("{src_cache}/src-entry")).exists());
}
