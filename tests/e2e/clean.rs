use crate::*;
use ::function_name::named;
use std::path::Path;
use std::process::Command;

fn run_with_cache_home(root: &str, cache_home: &str, args: &[&str]) -> Result<String, String> {
    let result = Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args(["-SVy", "-R", root, "-O", root])
        .args(args)
        .env("XDG_CACHE_HOME", cache_home)
        .output()
        .expect("failed to execute bpt");
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).to_string())
    }
}

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

    let stdout = run_with_cache_home(per_test_path!(), &cache_home, &["clean"]).unwrap();
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

    let stdout =
        run_with_cache_home(per_test_path!(), &cache_home, &["clean", "--packages"]).unwrap();
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

    let stdout =
        run_with_cache_home(per_test_path!(), &cache_home, &["clean", "--source"]).unwrap();
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

    let stdout = run_with_cache_home(per_test_path!(), &cache_home, &["clean", "-D"]).unwrap();
    assert!(stdout.contains("Dry run would have removed 2 cached items"));
    assert!(Path::new(&format!("{pkg_cache}/pkg-entry")).exists());
    assert!(Path::new(&format!("{src_cache}/src-entry")).exists());
}
