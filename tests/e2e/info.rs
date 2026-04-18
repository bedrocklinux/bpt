use crate::e2e::common::bbuild::write_modified_bbuild;
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn info_by_repository_partid() {
    setup_test!();

    let stdout = run!("info", "fakeblock").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("just a boolean driven aggregation"));
}

#[test]
#[named]
fn info_by_bbuild_path() {
    setup_test!();

    let stdout = run!("info", repo_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Name:         fakeblock"));
    assert!(stdout.contains("Architecture: bbuild"));
    assert!(stdout.contains("just a boolean driven aggregation"));
}

#[test]
#[named]
fn info_by_bbuild_url() {
    setup_test!();

    let url = repo_url!("fakeblock@1.0.0.bbuild");
    let stdout = run!("info", &url).unwrap();
    assert!(stdout.contains("Name:         fakeblock"));
    assert!(stdout.contains("Architecture: bbuild"));
    assert!(stdout.contains("just a boolean driven aggregation"));
}

#[test]
#[named]
fn info_bbuild_with_absolute_backup_path_errors() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "depends=\"fakeblock-songs>=1.0.0\"",
            "depends=\"fakeblock-songs>=1.0.0\"\nbackup=\"/etc/fakeblock.conf\"",
        )],
    );

    let result = run!("info", per_test_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("contains invalid Backup field"));
    assert!(stderr.contains("absolute paths are not allowed"));
    assert!(stderr.contains("/etc/fakeblock.conf"));
}

#[test]
#[named]
fn info_missing_pkg_errors() {
    setup_test!();

    let result = run!("info", "this-does-not-exist");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate available package"));
    assert!(stderr.contains("this-does-not-exist"));
}

#[test]
#[named]
fn info_installed_flag_restricts_partids_to_installed_packages() {
    setup_test!();

    let result = run!("info", "--installed", "fakeblock");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate installed package"));
    assert!(stderr.contains("fakeblock"));
}

#[test]
#[named]
fn info_by_installed_pkgid() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@2.0.0.bbuild"),
        &[
            ("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\""),
            (
                "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
                "pkgdesc=\"installed-only fakeblock variant\"",
            ),
        ],
    );
    let _ = run!("install", per_test_path!("fakeblock@2.0.0.bbuild")).unwrap();

    let stdout = run!("info", "fakeblock@2.0.0").unwrap();
    assert!(stdout.contains("Name:         fakeblock"));
    assert!(stdout.contains("Version:      2.0.0"));
    assert!(stdout.contains("Architecture: noarch"));
    assert!(stdout.contains("installed-only fakeblock variant"));
}

#[test]
#[named]
fn info_by_pkgid_prefers_installed_metadata_over_repository_metadata() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
            "pkgdesc=\"installed-only fakeblock variant\"",
        )],
    );
    let _ = run!("install", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();

    let installed_stdout = run!("info", "fakeblock@1.0.0").unwrap();
    assert!(installed_stdout.contains("installed-only fakeblock variant"));

    let repo_stdout = run!("info", repo_path!("fakeblock@1.0.0:noarch.bpt")).unwrap();
    assert!(repo_stdout.contains("just a boolean driven aggregation"));
    assert!(!repo_stdout.contains("installed-only fakeblock variant"));
}

#[test]
#[named]
fn info_repository_flag_prefers_repository_metadata_for_partids() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
            "pkgdesc=\"installed-only fakeblock variant\"",
        )],
    );
    let _ = run!("install", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();

    let stdout = run!("info", "--repository", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("just a boolean driven aggregation"));
    assert!(!stdout.contains("installed-only fakeblock variant"));
}

#[test]
#[named]
fn info_both_flags_still_prefers_installed_metadata_for_partids() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
            "pkgdesc=\"installed-only fakeblock variant\"",
        )],
    );
    let _ = run!("install", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();

    let stdout = run!("info", "--installed", "--repository", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("installed-only fakeblock variant"));
}

#[test]
#[named]
fn info_flags_do_not_affect_path_inputs() {
    setup_test!();

    let stdout = run!("info", "--repository", repo_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Name:         fakeblock"));
    assert!(stdout.contains("Architecture: bbuild"));
}

#[test]
#[named]
fn info_bbuild_lists_makebins_but_bpt_does_not() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[("makedepends=\"\"", "makedepends=\"\"\nmakebins=\"sh\"")],
    );

    let bbuild_info = run!("info", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(bbuild_info.contains("MakeBins"));
    assert!(bbuild_info.contains("sh"));

    let build_stdout = run!("build", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(build_stdout.contains("Built fakeblock@1.0.0:noarch.bpt"));

    let bpt_info = run!("info", per_test_path!("fakeblock@1.0.0:noarch.bpt")).unwrap();
    assert!(!bpt_info.contains("MakeBins"));
}

#[test]
#[named]
fn info_bbuild_lists_makebin_group_aliases() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "makedepends=\"\"",
            "makedepends=\"\"\nmakebins=\"@core @autotools\"",
        )],
    );

    let stdout = run!("info", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("MakeBins"));
    assert!(stdout.contains("@core"));
    assert!(stdout.contains("@autotools"));
}

#[test]
#[named]
fn info_bbuild_with_invalid_makebin_group_alias_errors() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[("makedepends=\"\"", "makedepends=\"\"\nmakebins=\"@bogus\"")],
    );

    let result = run!("info", per_test_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("contains invalid MakeBin field"));
    assert!(stderr.contains("Unrecognized group alias `@bogus`"));
}
