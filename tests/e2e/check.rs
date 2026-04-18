use crate::e2e::common::bbuild::write_modified_bbuild;
use crate::*;
use ::function_name::named;

fn rewrite_bpt_owner_to_current_ids(path: &str) {
    use nix::unistd::{getgid, getuid};
    use std::io::{Cursor, Read};

    const BPT_MAGIC: &[u8] = b"bpt\0";

    let uid = getuid().as_raw() as u64;
    let gid = getgid().as_raw() as u64;
    let bytes = std::fs::read(path).unwrap();
    assert!(
        bytes.starts_with(BPT_MAGIC),
        "expected `{path}` to begin with bpt magic"
    );

    let tarball = zstd::stream::decode_all(Cursor::new(&bytes[BPT_MAGIC.len()..])).unwrap();
    let mut archive = tar::Archive::new(Cursor::new(tarball));
    let mut rebuilt = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut rebuilt);
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let mut header = entry.header().clone();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            header.set_uid(uid);
            header.set_gid(gid);
            header.set_cksum();
            builder.append(&header, Cursor::new(data)).unwrap();
        }
        builder.finish().unwrap();
    }

    let compressed = zstd::stream::encode_all(Cursor::new(rebuilt), 0).unwrap();
    let mut out = Vec::with_capacity(BPT_MAGIC.len() + compressed.len());
    out.extend_from_slice(BPT_MAGIC);
    out.extend_from_slice(&compressed);
    std::fs::write(path, out).unwrap();
}

fn prepare_backup_fixture(root: &str) {
    let conf_path = format!("{root}etc/bpt/bpt.conf");
    let conf = std::fs::read_to_string(&conf_path).unwrap();
    let conf = conf.replace("tmp = /tmp", &format!("tmp = {root}tmp"));
    std::fs::write(conf_path, conf).unwrap();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        &format!("{root}fakeblock@1.0.0.bbuild"),
        &[(
            "depends=\"fakeblock-songs>=1.0.0\"",
            "depends=\"fakeblock-songs>=1.0.0\"\nbackup=\"etc/fakeblock.conf\"",
        )],
    );

    let songs_bbuild = repo_path!("fakeblock-songs@1.0.0.bbuild");
    let fakeblock_bbuild = format!("{root}fakeblock@1.0.0.bbuild");
    let _ = run_at!(root, "build", songs_bbuild).unwrap();
    let _ = run_at!(root, "build", &fakeblock_bbuild).unwrap();

    let songs_bpt = format!("{root}fakeblock-songs@1.0.0:noarch.bpt");
    let fakeblock_bpt = format!("{root}fakeblock@1.0.0:noarch.bpt");
    rewrite_bpt_owner_to_current_ids(&songs_bpt);
    rewrite_bpt_owner_to_current_ids(&fakeblock_bpt);

    let _ = run_at!(root, "install", &songs_bpt).unwrap();
    let _ = run_at!(root, "install", &fakeblock_bpt).unwrap();
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
    setup_test!();
    prepare_backup_fixture(per_test_path!());
    std::fs::write(per_test_path!("etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stdout = run!("check", "fakeblock").unwrap();
    assert!(stdout.contains("Warning:"));
    assert!(stdout.contains("Installed backup file differences"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Incorrect sha256:"));
    assert!(stdout.contains(per_test_path!("etc/fakeblock.conf")));
    assert!(stdout.contains("\nChecked installed package fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("Checked installed package fakeblock@1.0.0:noarch"));
}

#[test]
#[named]
fn check_backup_diff_strict_errors() {
    setup_test!();
    prepare_backup_fixture(per_test_path!());
    std::fs::write(per_test_path!("etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stderr = run!("check", "--strict", "fakeblock").unwrap_err();
    assert!(stderr.contains("Installed package integrity check failed"));
    assert!(stderr.contains("fakeblock@1.0.0:noarch"));
    assert!(stderr.contains("Incorrect sha256:"));
    assert!(stderr.contains(per_test_path!("etc/fakeblock.conf")));
}

#[test]
#[named]
fn check_backup_diff_ignore_backup_succeeds_silently() {
    setup_test!();
    prepare_backup_fixture(per_test_path!());
    std::fs::write(per_test_path!("etc/fakeblock.conf"), "sound=zap\n").unwrap();

    let stdout = run!("check", "--ignore-backup", "fakeblock").unwrap();
    assert!(!stdout.contains("Warning:"));
    assert!(!stdout.contains("Incorrect sha256:"));
    assert!(!stdout.contains("fakeblock.conf"));
    assert!(stdout.contains("Checked installed package fakeblock@1.0.0:noarch"));
}

#[test]
#[named]
fn check_rejects_strict_with_ignore_backup() {
    setup_test!();

    let stderr = run!("check", "--strict", "--ignore-backup").unwrap_err();
    assert!(stderr.contains("cannot be used with"));
    assert!(stderr.contains("--ignore-backup"));
}
