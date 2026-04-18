use crate::e2e::common::run::run_bpt_at_with_envs;
use crate::e2e::common::bbuild::write_modified_bbuild;
use crate::*;
use ::function_name::named;
use std::os::unix::fs::symlink;
use std::path::Path;

#[test]
#[named]
fn build_from_path() {
    setup_test!();

    let stdout = run!("build", repo_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_from_url() {
    setup_test!();

    let url = repo_url!("fakeblock@1.0.0.bbuild");
    let stdout = run!("build", &url).unwrap();
    assert!(stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_multiple_from_paths() {
    setup_test!();

    let stdout = run!(
        "build",
        repo_path!("fakeblock@1.0.0.bbuild"),
        repo_path!("fakeblock-song-gen@1.0.0.bbuild")
    )
    .unwrap();
    assert!(stdout.contains("Built all 2 packages"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(std::path::Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_arch_native_rejected() {
    setup_test!();

    let result = run!(
        "build",
        "-a",
        "native",
        repo_path!("fakeblock@1.0.0.bbuild")
    );
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("cannot output non-portable `:native` packages"));
}

#[test]
#[named]
fn build_arch_bbuild_rejected() {
    setup_test!();

    let result = run!(
        "build",
        "-a",
        "bbuild",
        repo_path!("fakeblock@1.0.0.bbuild")
    );
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("`bbuild` is a package build definition"));
}

#[test]
#[named]
fn build_output_exists_rejected() {
    setup_test!();

    std::fs::write(per_test_path!("fakeblock@1.0.0:noarch.bpt"), "already here").unwrap();

    let result = run!("build", repo_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Cannot build `fakeblock@1.0.0:noarch.bpt`"));
    assert!(stderr.contains("as something already exists"));
}

#[test]
#[named]
fn build_from_repo_pkgid() {
    setup_test!();

    let stdout = run!("build", "fakeblock").unwrap();
    assert!(stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_repo_pkgid_arch_overrides_cli_arch() {
    setup_test!();

    let stdout = run!("build", "-a", "native", "fakeblock:noarch").unwrap();
    assert!(stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_with_makedepends_from_repo() {
    setup_test!();

    let stdout = run!("build", repo_path!("fakeblock-songs@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Built fakeblock-songs@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock-songs@1.0.0:noarch.bpt")).exists());
    let files = run!("files", per_test_path!("fakeblock-songs@1.0.0:noarch.bpt")).unwrap();
    assert!(files.contains("usr/share/fakeblock/songs/main-theme"));
    assert!(files.contains("usr/share/fakeblock/songs/top-banana"));
    assert!(files.contains("usr/share/fakeblock/songs/boomerang"));
}

#[test]
#[named]
fn build_orders_requested_bbuilds_by_makedepends() {
    setup_test!();

    // Ensure only the explicitly requested bbuilds are available, so this depends on in-process
    // build ordering rather than repository prebuilt packages.
    std::fs::remove_dir_all(per_test_path!("var/lib/bpt/pkgidx")).unwrap();
    let consumer = repo_path!("aaa-consumer@1.0.0.bbuild");
    let helper = repo_path!("zzz-helper@1.0.0.bbuild");

    let stdout = run!("build", consumer, helper).unwrap();
    assert!(stdout.contains("Built all 2 packages"));
    assert!(Path::new(per_test_path!("aaa-consumer@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("zzz-helper@1.0.0:noarch.bpt")).exists());

    let files = run!("files", per_test_path!("aaa-consumer@1.0.0:noarch.bpt")).unwrap();
    assert!(files.contains("usr/share/aaa-consumer/generated.txt"));
}

#[test]
#[named]
fn build_repo_missing_pkg() {
    setup_test!();

    let result = run!("build", "this-does-not-exist");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate available package"));
    assert!(stderr.contains("this-does-not-exist"));
}

#[test]
#[named]
fn build_dry_run_from_path() {
    setup_test!();

    let stdout = run!("build", "-D", repo_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Would build fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Would build fakeblock@1.0.0:noarch\nDry ran build of 1 package(s)"));
    assert!(stdout.contains("Dry ran build of 1 package(s)"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_dry_run_multiple_from_paths() {
    setup_test!();

    let stdout = run!(
        "build",
        "-D",
        repo_path!("fakeblock@1.0.0.bbuild"),
        repo_path!("fakeblock-song-gen@1.0.0.bbuild")
    )
    .unwrap();
    assert!(stdout.contains("Would build fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Would build fakeblock-song-gen@1.0.0:noarch"));
    assert!(stdout.contains("Dry ran build of 2 package(s)"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(!Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_dry_run_output_exists_still_rejected() {
    setup_test!();

    std::fs::write(per_test_path!("fakeblock@1.0.0:noarch.bpt"), "already here").unwrap();

    let result = run!("build", "-D", repo_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Cannot build `fakeblock@1.0.0:noarch.bpt`"));
    assert!(stderr.contains("as something already exists"));
}

#[test]
#[named]
fn build_arch_override_to_noarch_when_supported() {
    setup_test!();

    let stdout = run!(
        "build",
        "-a",
        "x86_64",
        repo_path!("fakeblock@1.0.0.bbuild")
    )
    .unwrap();
    assert!(stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));
    assert!(std::path::Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_missing_makebins_errors() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "makedepends=\"\"",
            "makedepends=\"\"\nmakebins=\"bpt-definitely-missing-makebin-test\"",
        )],
    );

    let result = run!("build", per_test_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("required build-time binaries were not found in $PATH"));
    assert!(stderr.contains("bpt-definitely-missing-makebin-test"));
    assert!(stderr.contains("another package manager"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn build_makebin_group_alias_expands_to_missing_bins() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[("makedepends=\"\"", "makedepends=\"\"\nmakebins=\"@core\"")],
    );

    let bindir = per_test_path!("bin");
    std::fs::create_dir_all(&bindir).unwrap();
    symlink("/bin/sh", per_test_path!("bin/sh")).unwrap();

    let result = run_bpt_at_with_envs(
        per_test_path!(),
        &["build", per_test_path!("fakeblock@1.0.0.bbuild")],
        &[("PATH", bindir)],
    );
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("required build-time binaries were not found in $PATH"));
    assert!(stderr.contains("awk"));
    assert!(stderr.contains("cat"));
    assert!(!stderr.contains("@core"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}
