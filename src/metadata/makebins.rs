use crate::{
    error::*,
    io::{is_executable_in_paths, path_env},
    make_field_list,
    marshalling::FieldList,
    metadata::*,
};
use camino::Utf8PathBuf;
use std::collections::HashSet;

/// Binaries expected to be in the `$PATH` at build time
/// For example, a compiler.
#[derive(Clone, Debug)]
pub struct MakeBins(Vec<MakeBin>);

make_field_list!(MakeBins, PkgKey, MakeBin);

impl MakeBins {
    pub fn confirm_available(&self, pkgid: &PkgId) -> Result<(), Err> {
        self.confirm_available_in_paths(pkgid, &path_env())
    }

    fn confirm_available_in_paths(&self, pkgid: &PkgId, paths: &[Utf8PathBuf]) -> Result<(), Err> {
        let mut missing: HashSet<&str> = HashSet::new();
        let mut array = [""];

        for makebin in self.iter() {
            for bin in makebin.expanded(&mut array) {
                if !is_executable_in_paths(bin, paths) {
                    missing.insert(bin);
                }
            }
        }

        if missing.is_empty() {
            return Ok(());
        }

        let mut missing: Vec<&str> = missing.into_iter().collect();
        missing.sort_unstable();
        let missing = missing.join(", ");

        Err(Err::MakeBinsMissingInPath(pkgid.to_string(), missing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshalling::{AsBytes, FieldList, FieldStr, FromFieldStr};
    use crate::{constant::CORE_MAKEBINS, testutil::unit_test_tmp_dir};
    use std::{fs, os::unix::prelude::PermissionsExt};

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("makebins", name)
    }

    fn test_pkgid() -> PkgId {
        PkgId::new(
            PkgName::try_from("test-pkg").unwrap(),
            PkgVer::try_from("1.2.3").unwrap(),
            Arch::noarch,
        )
    }

    fn create_executable(dir: &Utf8PathBuf, name: &str) {
        let path = dir.join(name);
        fs::write(&path, "#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }

    #[test]
    fn roundtrip_multiple_entries() {
        let makebins =
            MakeBins::from_field_str(FieldStr::try_from("make pkg-config").unwrap()).unwrap();

        assert_eq!(makebins.as_slice().len(), 2);
        assert_eq!(makebins.as_slice()[0].as_str(), "make");
        assert_eq!(makebins.as_slice()[1].as_str(), "pkg-config");
        assert_eq!(
            std::str::from_utf8(AsBytes::as_bytes(&makebins).as_ref()).unwrap(),
            "make pkg-config"
        );
    }

    #[test]
    fn empty_is_empty_list() {
        let makebins = MakeBins::from_field_str(FieldStr::empty()).unwrap();
        assert!(makebins.as_slice().is_empty());
    }

    #[test]
    fn confirm_available_accepts_custom_paths_for_single_makebin() {
        let dir = test_dir("confirm_available_accepts_custom_paths_for_single_makebin");
        create_executable(&dir, "make");
        let makebins = MakeBins::from_field_str(FieldStr::try_from("make").unwrap()).unwrap();

        makebins
            .confirm_available_in_paths(&test_pkgid(), &[dir])
            .unwrap();
    }

    #[test]
    fn confirm_available_expands_group_aliases() {
        let dir = test_dir("confirm_available_expands_group_aliases");
        for name in CORE_MAKEBINS {
            create_executable(&dir, name);
        }

        let makebins = MakeBins::from_field_str(FieldStr::try_from("@core").unwrap()).unwrap();
        makebins
            .confirm_available_in_paths(&test_pkgid(), &[dir])
            .unwrap();
    }

    #[test]
    fn confirm_available_deduplicates_missing_bins_from_groups_and_singles() {
        let makebins =
            MakeBins::from_field_str(FieldStr::try_from("@autotools m4").unwrap()).unwrap();

        let err = makebins
            .confirm_available_in_paths(&test_pkgid(), &[])
            .unwrap_err();

        let Err::MakeBinsMissingInPath(_, missing) = err else {
            panic!("unexpected error variant");
        };
        let entries: Vec<&str> = missing.split(", ").collect();
        assert_eq!(entries.iter().filter(|&&entry| entry == "m4").count(), 1);
    }
}
