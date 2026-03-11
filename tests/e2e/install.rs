use crate::e2e::instpkg_testutil::{read, write_modified_bbuild};
use crate::*;
use ::function_name::named;
use std::path::Path;

#[test]
#[named]
fn install_repository_pkg_installs_dependencies_and_marks_world_explicit() {
    setup_test!();

    let stdout = run!("install", "fakeblock").unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock"));
    assert!(!world.contains("fakeblock-songs"));

    assert!(
        Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(
        Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock-songs@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(Path::new(per_test_path!("usr/bin/fakeblock")).exists());
    assert!(Path::new(per_test_path!("usr/share/fakeblock/songs/main-theme")).exists());
    assert!(!Path::new(per_test_path!("usr/bin/fakeblock-song-gen")).exists());
}

#[test]
#[named]
fn install_bbuild_path_pins_arch_in_world_and_builds() {
    setup_test!();

    let stdout = run!("install", repo_path!("fakeblock.bbuild")).unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("build"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock:noarch"));
    assert!(!world.contains("fakeblock@1.0.0"));
}

#[test]
#[named]
fn install_bpt_url_pins_arch_in_world() {
    setup_test!();

    let url = repo_url!("fakeblock@1.0.0:noarch.bpt");
    let stdout = run!("install", &url).unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock:noarch"));
    assert!(!world.contains("fakeblock@1.0.0"));
}

#[test]
#[named]
fn install_dry_run_does_not_mutate_state() {
    setup_test!();

    let stdout = run!("install", "-D", "fakeblock").unwrap();
    assert!(stdout.contains("Would have:"));
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("Dry ran updated installed package set"));

    assert!(!Path::new(per_test_path!("etc/bpt/world")).exists());
    assert!(!Path::new(per_test_path!("var/lib/bpt/instpkg")).exists());
    assert!(!Path::new(per_test_path!("usr/bin/fakeblock")).exists());
}

#[test]
#[named]
fn install_dependency_make_explicit_uses_retain() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let stdout = run!("install", "fakeblock-songs").unwrap();
    assert!(stdout.contains("Retain"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stdout.contains("world add fakeblock-songs"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock"));
    assert!(world.contains("fakeblock-songs"));
}

#[test]
#[named]
fn install_reinstall_dependency_only_upgrades_same_version_without_world_add() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let stdout = run!("install", "--reinstall", "fakeblock-songs").unwrap();
    assert!(stdout.contains("Upgrade"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stdout.contains("from fakeblock-songs@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock"));
    assert!(!world.contains("fakeblock-songs"));

    let dependency = run!("list", "--dependency").unwrap();
    assert!(dependency.contains("fakeblock-songs@1.0.0:noarch"));
}

#[test]
#[named]
fn install_bpt_path_pins_arch_in_world() {
    setup_test!();

    let _ = run!("build", repo_path!("fakeblock.bbuild")).unwrap();
    let stdout = run!("install", per_test_path!("fakeblock@1.0.0:noarch.bpt")).unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock:noarch"));
    assert!(!world.contains("fakeblock@1.0.0"));
}

#[test]
#[named]
fn install_conflicting_bpt_errors_without_mutating_state() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    std::fs::write(
        per_test_path!("fakeblock-conflict.bbuild"),
        r#"#!/bin/sh
pkgname="fakeblock-conflict"
pkgver="1.0.0"
pkgdesc="conflicts on /usr/bin/fakeblock"
homepage="N/A"
license="MIT"
depends=""
backup=""
makearch="noarch"
makedepends=""
source=""
sha256sums=""

build() {
    mkdir -p "${pkgdir}/usr/bin"
    printf '%s\n' '#!/bin/sh' 'printf "conflict\n"' > "${pkgdir}/usr/bin/fakeblock"
    chmod a+rx "${pkgdir}/usr/bin/fakeblock"
}
"#,
    )
    .unwrap();
    let _ = run!("build", per_test_path!("fakeblock-conflict.bbuild")).unwrap();

    let result = run!(
        "install",
        per_test_path!("fakeblock-conflict@1.0.0:noarch.bpt")
    );
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("File path conflict"));
    assert!(stderr.contains("usr/bin/fakeblock"));
    assert!(stderr.contains("fakeblock-conflict@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert_eq!(world.trim(), "fakeblock");
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock-conflict@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
}

#[test]
#[named]
fn install_local_bpt_bootstraps_missing_bpt_infrastructure() {
    *crate::e2e::common::setup::COMMON_SETUP;

    let root = per_test_path!("fresh-root");
    std::fs::remove_dir_all(root).unwrap_or(());

    let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args([
            "-SVy",
            "-R",
            root,
            "-O",
            root,
            "install",
            repo_path!("fakeblock-songs@1.0.0:noarch.bpt"),
        ])
        .output()
        .expect("failed to execute bpt");

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));

    let world = read(per_test_path!("fresh-root/etc/bpt/world"));
    assert!(world.contains("fakeblock-songs:noarch"));
    assert!(
        Path::new(per_test_path!(
            "fresh-root/usr/share/fakeblock/songs/main-theme"
        ))
        .exists()
    );
}

#[test]
#[named]
fn install_backup_file_identical_does_not_create_bptnew() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock-backup.bbuild"),
        &[(
            "depends=\"fakeblock-songs>=1.0.0\"",
            "depends=\"fakeblock-songs>=1.0.0\"\nbackup=\"etc/fakeblock.conf\"",
        )],
    );
    std::fs::write(per_test_path!("etc/fakeblock.conf"), "sound=tok\n").unwrap();

    let stdout = run!("install", per_test_path!("fakeblock-backup.bbuild")).unwrap();
    assert!(stdout.contains("Install"));
    assert!(!stdout.contains(".bptnew"));
    assert!(!Path::new(per_test_path!("etc/fakeblock.conf.bptnew")).exists());
}

#[test]
#[named]
fn install_backup_file_difference_creates_bptnew() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock-backup.bbuild"),
        &[(
            "depends=\"fakeblock-songs>=1.0.0\"",
            "depends=\"fakeblock-songs>=1.0.0\"\nbackup=\"etc/fakeblock.conf\"",
        )],
    );
    std::fs::write(per_test_path!("etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stdout = run!("install", per_test_path!("fakeblock-backup.bbuild")).unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("Created"));
    assert!(stdout.contains("etc/fakeblock.conf.bptnew"));
    assert!(Path::new(per_test_path!("etc/fakeblock.conf.bptnew")).exists());
}
