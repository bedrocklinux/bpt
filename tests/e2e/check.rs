use crate::e2e::instpkg_testutil::{rewrite_bpt_owner_to_current_ids, write_modified_bbuild};
use crate::*;
use ::function_name::named;
use std::process::Command;

fn custom_root(name: &str) -> String {
    format!("/tmp/bpt-check-{}-{name}", std::process::id())
}

fn setup_custom_root(root: &str) {
    *crate::e2e::common::setup::COMMON_SETUP;
    std::fs::remove_dir_all(root).unwrap_or(());
    crate::e2e::common::setup::copy_dir(common_path!("etc/bpt"), &format!("{root}/etc/bpt"));
    crate::e2e::common::setup::copy_dir(common_path!("var"), &format!("{root}/var"));
    let conf_path = format!("{root}/etc/bpt/bpt.conf");
    let conf = std::fs::read_to_string(&conf_path).unwrap();
    let conf = conf.replace("tmp = /tmp", &format!("tmp = {root}/tmp"));
    std::fs::write(conf_path, conf).unwrap();
}

fn run_at_root(root: &str, args: &[&str]) -> Result<String, String> {
    let result = Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args(["-SVy", "-R", root, "-O", root])
        .args(args)
        .output()
        .expect("failed to execute bpt");
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).to_string())
    }
}

fn prepare_backup_fixture(root: &str) {
    setup_custom_root(root);

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        &format!("{root}/fakeblock@1.0.0.bbuild"),
        &[(
            "depends=\"fakeblock-songs>=1.0.0\"",
            "depends=\"fakeblock-songs>=1.0.0\"\nbackup=\"etc/fakeblock.conf\"",
        )],
    );

    let songs_bbuild = repo_path!("fakeblock-songs@1.0.0.bbuild");
    let fakeblock_bbuild = format!("{root}/fakeblock@1.0.0.bbuild");
    let _ = run_at_root(root, &["build", songs_bbuild]).unwrap();
    let _ = run_at_root(root, &["build", &fakeblock_bbuild]).unwrap();

    let songs_bpt = format!("{root}/fakeblock-songs@1.0.0:noarch.bpt");
    let fakeblock_bpt = format!("{root}/fakeblock@1.0.0:noarch.bpt");
    rewrite_bpt_owner_to_current_ids(&songs_bpt);
    rewrite_bpt_owner_to_current_ids(&fakeblock_bpt);

    let _ = run_at_root(root, &["install", &songs_bpt]).unwrap();
    let _ = run_at_root(root, &["install", &fakeblock_bpt]).unwrap();
}

#[test]
#[named]
fn check_no_installed_packages_is_noop() {
    setup_test!();

    let stdout = run!("check").unwrap();
    assert!(stdout.contains("No installed packages to check"));
}

#[test]
#[named]
fn check_all_installed_packages_reports_metadata_mismatches() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    let result = run!("check");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Installed package integrity check failed"));
    assert!(stderr.contains("fakeblock@1.0.0:noarch"));
    assert!(stderr.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stderr.contains("Incorrect uid:"));
    assert!(stderr.contains("Incorrect gid:"));
}

#[test]
#[named]
fn check_reports_issues_across_multiple_packages() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    std::fs::remove_file(per_test_path!("usr/bin/fakeblock")).unwrap();
    std::fs::write(
        per_test_path!("usr/share/fakeblock/songs/main-theme"),
        "modified song data\n",
    )
    .unwrap();

    let result = run!("check");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Installed package integrity check failed"));
    assert!(stderr.contains("fakeblock@1.0.0:noarch"));
    assert!(stderr.contains("Missing:"));
    assert!(stderr.contains(per_test_path!("usr/bin/fakeblock")));
    assert!(stderr.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stderr.contains("Incorrect sha256:"));
    assert!(stderr.contains(per_test_path!("usr/share/fakeblock/songs/main-theme")));
}

#[test]
#[named]
fn check_selected_pkg_filters_other_broken_pkg() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    std::fs::write(
        per_test_path!("usr/share/fakeblock/songs/main-theme"),
        "modified song data\n",
    )
    .unwrap();

    let fakeblock = run!("check", "fakeblock").unwrap_err();
    assert!(fakeblock.contains("fakeblock@1.0.0:noarch"));
    assert!(!fakeblock.contains("fakeblock-songs@1.0.0:noarch\n"));
    assert!(fakeblock.contains("Incorrect uid:"));

    let fakeblock_songs = run!("check", "fakeblock-songs").unwrap_err();
    assert!(fakeblock_songs.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(!fakeblock_songs.contains("fakeblock@1.0.0:noarch\n"));
    assert!(fakeblock_songs.contains("Incorrect sha256:"));
}

#[test]
#[named]
fn check_missing_requested_pkg_errors() {
    setup_test!();

    let result = run!("check", "fakeblock");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate installed package"));
    assert!(stderr.contains("fakeblock"));
}

#[test]
#[named]
fn check_backup_diff_warns_but_succeeds() {
    let root = custom_root(function_name!());
    prepare_backup_fixture(&root);
    std::fs::write(format!("{root}/etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stdout = run_at_root(&root, &["check", "fakeblock"]).unwrap();
    assert!(stdout.contains("Warning:"));
    assert!(stdout.contains("Installed backup file differences"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Incorrect sha256:"));
    assert!(stdout.contains(&format!("{root}/etc/fakeblock.conf")));
    assert!(stdout.contains("\nChecked installed package fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Checked installed package fakeblock@1.0.0:noarch"));
}

#[test]
#[named]
fn check_backup_diff_strict_errors() {
    let root = custom_root(function_name!());
    prepare_backup_fixture(&root);
    std::fs::write(format!("{root}/etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stderr = run_at_root(&root, &["check", "--strict", "fakeblock"]).unwrap_err();
    assert!(stderr.contains("Installed package integrity check failed"));
    assert!(stderr.contains("fakeblock@1.0.0:noarch"));
    assert!(stderr.contains("Incorrect sha256:"));
    assert!(stderr.contains(&format!("{root}/etc/fakeblock.conf")));
}

#[test]
#[named]
fn check_backup_diff_ignore_backup_succeeds_silently() {
    let root = custom_root(function_name!());
    prepare_backup_fixture(&root);
    std::fs::write(format!("{root}/etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stdout = run_at_root(&root, &["check", "--ignore-backup", "fakeblock"]).unwrap();
    assert!(!stdout.contains("Warning:"));
    assert!(!stdout.contains("Incorrect sha256:"));
    assert!(!stdout.contains("fakeblock.conf"));
    assert!(stdout.contains("Checked installed package fakeblock@1.0.0:noarch"));
}

#[test]
#[named]
fn check_rejects_strict_with_ignore_backup() {
    let root = custom_root(function_name!());
    setup_custom_root(&root);

    let stderr = run_at_root(&root, &["check", "--strict", "--ignore-backup"]).unwrap_err();
    assert!(stderr.contains("cannot be used with"));
    assert!(stderr.contains("--ignore-backup"));
}
