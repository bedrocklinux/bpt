//! Unit tests for installed-package reconciliation internals.

use crate::collection::{AvailableBpts, InstalledPkgs};
use crate::constant::{INSTPKG_DIR_PATH, TARBALL_PKGINFO_PATH};
use crate::error::Err;
use crate::file::{Bpt, PrivKey};
use crate::location::RootDir;
use crate::marshalling::{FieldList, FieldStr, FromFieldStr, Serialize};
use crate::metadata::{
    Arch, Backup, Depends, Homepage, License, MakeArchs, MakeBins, MakeDepends, PartId, PkgDesc,
    PkgId, PkgInfo, PkgName, PkgVer, RepoPath,
};
use crate::reconcile::instpkg_rec::{InstPkgPlan, InstPkgReconciler, PreparedInstallOp};
use crate::testutil::unit_test_tmp_dir;
use camino::Utf8Path;
use std::str::FromStr;

fn test_root(name: &str) -> (camino::Utf8PathBuf, RootDir) {
    let tmp = unit_test_tmp_dir("instpkg_rec", name);
    let root = RootDir::from_path(&tmp);
    (tmp, root)
}

fn make_pkginfo(pkgname: &str, pkgver: &str, arch: Arch) -> PkgInfo {
    PkgInfo {
        pkgid: PkgId::new(
            PkgName::try_from(pkgname).unwrap(),
            PkgVer::try_from(pkgver).unwrap(),
            arch,
        ),
        pkgdesc: PkgDesc::from_field_str(FieldStr::try_from("test package").unwrap()).unwrap(),
        homepage: Homepage::from_field_str(FieldStr::try_from("https://example.com").unwrap())
            .unwrap(),
        license: License::from_field_str(FieldStr::try_from("MIT").unwrap()).unwrap(),
        backup: Backup::new(),
        depends: Depends::new(),
        makearchs: MakeArchs::new(),
        makebins: MakeBins::new(),
        makedepends: MakeDepends::new(),
        repopath: RepoPath::empty(),
    }
}

fn write_pkginfo(pkgdir: &Utf8Path, pkginfo: &PkgInfo) {
    let mut bytes = Vec::new();
    pkginfo.serialize(&mut bytes).unwrap();
    std::fs::write(pkgdir.join(TARBALL_PKGINFO_PATH), bytes).unwrap();
}

fn make_bpt_with_entries(
    test_name: &str,
    pkgname: &str,
    pkgver: &str,
    dirs: &[&str],
    files: &[(&str, &str)],
) -> Bpt {
    let tmp = unit_test_tmp_dir("instpkg_rec_bpt", test_name);
    let pkgdir = tmp.join("pkgdir");
    let outdir = tmp.join("out");
    std::fs::create_dir_all(&pkgdir).unwrap();
    std::fs::create_dir_all(&outdir).unwrap();

    let pkginfo = make_pkginfo(pkgname, pkgver, Arch::noarch);
    write_pkginfo(&pkgdir, &pkginfo);

    for dir in dirs {
        std::fs::create_dir_all(pkgdir.join(dir)).unwrap();
    }
    for (path, content) in files {
        let full = pkgdir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    Bpt::from_dir(
        pkgdir.as_path(),
        outdir.as_path(),
        &PrivKey::from_test_key(),
    )
    .unwrap()
}

fn install_bpt_for_test(root: &RootDir, bpt: &mut Bpt) {
    let instpkg_dir = root.as_path().join(INSTPKG_DIR_PATH);
    std::fs::create_dir_all(&instpkg_dir).unwrap();
    let mut bptnew = Vec::new();
    bpt.install(root, instpkg_dir.as_path(), &mut bptnew)
        .unwrap();
}

#[test]
fn world_remove_matches_name_only_entry() {
    let remove = PartId::from_str("foo").unwrap();
    let entry = PartId::from_str("foo:x86_64").unwrap();
    assert!(InstPkgReconciler::world_remove_matches(&remove, &entry));
}

#[test]
fn world_remove_matches_version_to_unpinned_entry() {
    let remove = PartId::from_str("foo@1.0.0").unwrap();
    let entry = PartId::from_str("foo").unwrap();
    assert!(InstPkgReconciler::world_remove_matches(&remove, &entry));
}

#[test]
fn world_remove_rejects_conflicting_version() {
    let remove = PartId::from_str("foo@1.0.0").unwrap();
    let entry = PartId::from_str("foo@2.0.0").unwrap();
    assert!(!InstPkgReconciler::world_remove_matches(&remove, &entry));
}

#[test]
fn world_remove_rejects_conflicting_arch() {
    let remove = PartId::from_str("foo:x86_64").unwrap();
    let entry = PartId::from_str("foo:aarch64").unwrap();
    assert!(!InstPkgReconciler::world_remove_matches(&remove, &entry));
}

#[test]
fn conflict_check_rejects_new_file_conflict_between_packages() {
    let (_tmp, root) = test_root("conflict_check_rejects_new_file_conflict_between_packages");
    let installed = InstalledPkgs::from_root_path_ro(&root).unwrap();
    let mut available = AvailableBpts::new();
    let pkg1 = make_bpt_with_entries(
        "conflict_check_rejects_new_file_conflict_between_packages_pkg1",
        "alpha",
        "1.0.0",
        &[],
        &[("usr/bin/shared", "one")],
    );
    let pkg2 = make_bpt_with_entries(
        "conflict_check_rejects_new_file_conflict_between_packages_pkg2",
        "beta",
        "1.0.0",
        &[],
        &[("usr/bin/shared", "two")],
    );
    let pkg1_id = pkg1.pkgid().clone();
    let pkg2_id = pkg2.pkgid().clone();
    available.add(pkg1);
    available.add(pkg2);

    let err = InstPkgPlan::conflict_check(
        &installed,
        &[],
        &[
            PreparedInstallOp {
                pkgid: pkg1_id.clone(),
            },
            PreparedInstallOp {
                pkgid: pkg2_id.clone(),
            },
        ],
        &[],
        &[],
        &available,
    )
    .unwrap_err();

    match err {
        Err::InstallConflict(path, left, right) => {
            assert_eq!(path, "usr/bin/shared");
            assert!(
                (*left == pkg1_id && *right == pkg2_id) || (*left == pkg2_id && *right == pkg1_id)
            );
        }
        other => panic!("expected InstallConflict, got {other}"),
    }
}

#[test]
fn conflict_check_rejects_conflict_with_installed_package() {
    let (_tmp, root) = test_root("conflict_check_rejects_conflict_with_installed_package");
    let mut installed_bpt = make_bpt_with_entries(
        "conflict_check_rejects_conflict_with_installed_package_installed",
        "installed-pkg",
        "1.0.0",
        &[],
        &[("usr/bin/shared", "installed")],
    );
    let installed_id = installed_bpt.pkgid().clone();
    install_bpt_for_test(&root, &mut installed_bpt);
    let installed = InstalledPkgs::from_root_path_ro(&root).unwrap();

    let mut available = AvailableBpts::new();
    let new_bpt = make_bpt_with_entries(
        "conflict_check_rejects_conflict_with_installed_package_new",
        "new-pkg",
        "1.0.0",
        &[],
        &[("usr/bin/shared", "new")],
    );
    let new_id = new_bpt.pkgid().clone();
    available.add(new_bpt);

    let err = InstPkgPlan::conflict_check(
        &installed,
        &[],
        &[PreparedInstallOp {
            pkgid: new_id.clone(),
        }],
        &[],
        &[],
        &available,
    )
    .unwrap_err();

    match err {
        Err::InstallConflict(path, left, right) => {
            assert_eq!(path, "usr/bin/shared");
            assert!(
                (*left == new_id && *right == installed_id)
                    || (*left == installed_id && *right == new_id)
            );
        }
        other => panic!("expected InstallConflict, got {other}"),
    }
}

#[test]
fn conflict_check_allows_shared_directory_paths() {
    let (_tmp, root) = test_root("conflict_check_allows_shared_directory_paths");
    let installed = InstalledPkgs::from_root_path_ro(&root).unwrap();
    let mut available = AvailableBpts::new();
    let pkg1 = make_bpt_with_entries(
        "conflict_check_allows_shared_directory_paths_pkg1",
        "alpha-dir",
        "1.0.0",
        &["usr/share/shared"],
        &[],
    );
    let pkg2 = make_bpt_with_entries(
        "conflict_check_allows_shared_directory_paths_pkg2",
        "beta-dir",
        "1.0.0",
        &["usr/share/shared"],
        &[],
    );
    let pkg1_id = pkg1.pkgid().clone();
    let pkg2_id = pkg2.pkgid().clone();
    available.add(pkg1);
    available.add(pkg2);

    InstPkgPlan::conflict_check(
        &installed,
        &[],
        &[
            PreparedInstallOp { pkgid: pkg1_id },
            PreparedInstallOp { pkgid: pkg2_id },
        ],
        &[],
        &[],
        &available,
    )
    .unwrap();
}

#[test]
fn conflict_check_rejects_directory_file_path_overlap() {
    let (_tmp, root) = test_root("conflict_check_rejects_directory_file_path_overlap");
    let installed = InstalledPkgs::from_root_path_ro(&root).unwrap();
    let mut available = AvailableBpts::new();
    let dir_pkg = make_bpt_with_entries(
        "conflict_check_rejects_directory_file_path_overlap_dir",
        "dir-pkg",
        "1.0.0",
        &["usr/share/shared"],
        &[],
    );
    let file_pkg = make_bpt_with_entries(
        "conflict_check_rejects_directory_file_path_overlap_file",
        "file-pkg",
        "1.0.0",
        &[],
        &[("usr/share/shared", "not a directory")],
    );
    let dir_id = dir_pkg.pkgid().clone();
    let file_id = file_pkg.pkgid().clone();
    available.add(dir_pkg);
    available.add(file_pkg);

    let err = InstPkgPlan::conflict_check(
        &installed,
        &[],
        &[
            PreparedInstallOp {
                pkgid: dir_id.clone(),
            },
            PreparedInstallOp {
                pkgid: file_id.clone(),
            },
        ],
        &[],
        &[],
        &available,
    )
    .unwrap_err();

    match err {
        Err::InstallConflict(path, left, right) => {
            assert_eq!(path, "usr/share/shared");
            assert!(
                (*left == dir_id && *right == file_id) || (*left == file_id && *right == dir_id)
            );
        }
        other => panic!("expected InstallConflict, got {other}"),
    }
}
