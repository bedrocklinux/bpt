use crate::{constant::*, error::*, io::*, location::*, metadata::*, str::*};
use camino::Utf8PathBuf;
use std::{
    collections::HashSet,
    fs::{File, rename},
    io::{ErrorKind, Write},
    str::FromStr,
};

pub struct World {
    // World file may not exist, in which case we have nothing to lock
    _lock: Option<File>,
    // Store the path so we can update the file without re-requesting the root
    path: Utf8PathBuf,
    // Store the lines of the file, mapped to their package.
    // This allows us to write the file back out without losing whitespace or comments.
    contents: Vec<(String, Option<PartId>)>,
    // The actual list of entries
    entries: HashSet<PartId>,
    // Whether we hold an exclusive lock and can save
    writable: bool,
}

impl World {
    pub fn from_root_path_ro(root: &RootDir) -> Result<Self, Err> {
        Self::new(root, false)
    }

    pub fn from_root_path_rw(root: &RootDir) -> Result<Self, Err> {
        Self::new(root, true)
    }

    fn new(root: &RootDir, writable: bool) -> Result<Self, Err> {
        let path = root.as_path().join(WORLD_PATH);

        if writable {
            let dir_path = path.as_str().strip_filename();
            std::fs::create_dir_all(dir_path)
                .map_err(|e| Err::CreateDir(dir_path.to_string(), e))?;
        }

        let mut file = match if writable {
            File::create_or_open_rw(&path)
        } else {
            File::open_ro(&path)
        } {
            Ok(file) => file,
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound && !writable => {
                return Ok(Self {
                    _lock: None,
                    path,
                    contents: Vec::new(),
                    entries: HashSet::new(),
                    writable,
                });
            }
            Err(e) => return Err(e),
        };

        if writable {
            file.lock_rw("world file").loc(path.clone())?;
        } else {
            file.lock_ro("world file").loc(path.clone())?;
        }

        let mut contents = Vec::new();
        let mut entries = HashSet::new();

        for line in file.read_small_file_string().loc(&path)?.lines() {
            let precomment = line.strip_comment();

            if precomment.is_empty() {
                contents.push((line.to_owned(), None));
            } else {
                let partid = PartId::from_str(precomment)?;
                entries.insert(partid.clone());
                contents.push((line.to_owned(), Some(partid)));
            }
        }

        Ok(Self {
            _lock: Some(file),
            path,
            contents,
            entries,
            writable,
        })
    }

    pub fn get_match(&self, pkgid: &PkgId) -> Option<&PartId> {
        self.entries.iter().find(|partid| partid.matches(pkgid))
    }

    pub fn contains_match(&self, pkgid: &PkgId) -> bool {
        self.get_match(pkgid).is_some()
    }

    #[cfg(test)]
    pub fn contains_entry(&self, partid: &PartId) -> bool {
        self.entries.contains(partid)
    }

    pub fn entries(&self) -> &HashSet<PartId> {
        &self.entries
    }

    pub fn replace_entries(&mut self, entries: HashSet<PartId>) {
        self.entries = entries;
    }

    // TODO: Remove when we have actual update mechanisms
    #[cfg(test)]
    pub fn entries_mut(&mut self) -> &mut HashSet<PartId> {
        &mut self.entries
    }

    pub fn save(&mut self) -> Result<(), Err> {
        debug_assert!(self.writable, "save() called on read-only World");

        let dir_path = self.path.as_str().strip_filename();
        std::fs::create_dir_all(dir_path).map_err(|e| Err::CreateDir(dir_path.to_string(), e))?;

        let tmp_path = self.path.with_file_name(".world-new");
        let _ = std::fs::remove_file(&tmp_path);
        let mut file = File::create_rw(&tmp_path)?;

        // Remove any lines that are no longer in the entries list
        self.contents.retain(|(_, partid)| {
            if let Some(partid) = partid {
                self.entries.contains(partid)
            } else {
                true
            }
        });

        // Add any new entries to the buffer
        for entry in &self.entries {
            if !self
                .contents
                .iter()
                .filter_map(|(_, partid)| partid.as_ref())
                .any(|p| p == entry)
            {
                self.contents.push((entry.to_string(), Some(entry.clone())));
            }
        }

        // Write out new buffer
        for (line, _) in &self.contents {
            writeln!(file, "{line}").map_err(|e| Err::Write(tmp_path.to_string(), e))?;
        }

        rename(&tmp_path, &self.path)
            .map_err(|e| Err::Rename(tmp_path.clone().into(), self.path.clone().into(), e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{constant::*, testutil::unit_test_tmp_dir};
    use std::str::FromStr;

    use super::*;

    fn test_root(name: &str) -> (Utf8PathBuf, RootDir) {
        let tmp = unit_test_tmp_dir("world", name);
        let root = RootDir::from_path(&tmp);
        (tmp, root)
    }

    fn write_world(root: &RootDir, content: &str) {
        let world_dir = root.as_path().join("etc/bpt");
        std::fs::create_dir_all(&world_dir).unwrap();
        std::fs::write(root.as_path().join(WORLD_PATH), content).unwrap();
    }

    #[test]
    fn ro_missing_file_returns_empty() {
        let (_tmp, root) = test_root("ro_missing_file_returns_empty");
        let world = World::from_root_path_ro(&root).unwrap();
        assert!(world.entries().is_empty());
    }

    #[test]
    fn ro_empty_file() {
        let (_tmp, root) = test_root("ro_empty_file");
        write_world(&root, "");
        let world = World::from_root_path_ro(&root).unwrap();
        assert!(world.entries().is_empty());
    }

    #[test]
    fn ro_parses_entries() {
        let (_tmp, root) = test_root("ro_parses_entries");
        write_world(&root, "bpt@1.0.0:x86_64\ngcc@11.2.0:aarch64\n");
        let world = World::from_root_path_ro(&root).unwrap();
        assert_eq!(world.entries().len(), 2);
        assert!(world.contains_entry(&PartId::from_str("bpt@1.0.0:x86_64").unwrap()));
        assert!(world.contains_entry(&PartId::from_str("gcc@11.2.0:aarch64").unwrap()));
    }

    #[test]
    fn ro_preserves_comments_and_blanks() {
        let (_tmp, root) = test_root("ro_preserves_comments_and_blanks");
        write_world(&root, "# header comment\n\nbpt@1.0.0:x86_64\n");
        let world = World::from_root_path_ro(&root).unwrap();
        assert_eq!(world.entries().len(), 1);
        assert_eq!(world.contents.len(), 3);
        assert!(world.contents[0].1.is_none());
        assert!(world.contents[1].1.is_none());
        assert!(world.contents[2].1.is_some());
    }

    #[test]
    fn ro_strips_inline_comments() {
        let (_tmp, root) = test_root("ro_strips_inline_comments");
        write_world(&root, "bpt # explicitly requested\n");
        let world = World::from_root_path_ro(&root).unwrap();
        assert_eq!(world.entries().len(), 1);
        assert!(world.contains_entry(&PartId::from_str("bpt").unwrap()));
    }

    #[test]
    fn ro_name_only_partid() {
        let (_tmp, root) = test_root("ro_name_only_partid");
        write_world(&root, "bpt\n");
        let world = World::from_root_path_ro(&root).unwrap();
        let partid = PartId::from_str("bpt").unwrap();
        assert!(world.contains_entry(&partid));
    }

    #[test]
    fn contains_match_name_only_matches_any_version_arch() {
        let (_tmp, root) = test_root("contains_match_name_only_matches_any_version_arch");
        write_world(&root, "bpt\n");
        let world = World::from_root_path_ro(&root).unwrap();
        let pkgid = PkgId::new(
            PkgName::try_from("bpt").unwrap(),
            PkgVer::try_from("2.0.0").unwrap(),
            Arch::from_str("aarch64").unwrap(),
        );
        assert!(world.contains_match(&pkgid));
    }

    #[test]
    fn contains_match_full_partid_requires_exact() {
        let (_tmp, root) = test_root("contains_match_full_partid_requires_exact");
        write_world(&root, "bpt@1.0.0:x86_64\n");
        let world = World::from_root_path_ro(&root).unwrap();

        let matching = PkgId::new(
            PkgName::try_from("bpt").unwrap(),
            PkgVer::try_from("1.0.0").unwrap(),
            Arch::from_str("x86_64").unwrap(),
        );
        assert!(world.contains_match(&matching));

        let wrong_ver = PkgId::new(
            PkgName::try_from("bpt").unwrap(),
            PkgVer::try_from("2.0.0").unwrap(),
            Arch::from_str("x86_64").unwrap(),
        );
        assert!(!world.contains_match(&wrong_ver));

        let wrong_arch = PkgId::new(
            PkgName::try_from("bpt").unwrap(),
            PkgVer::try_from("1.0.0").unwrap(),
            Arch::from_str("aarch64").unwrap(),
        );
        assert!(!world.contains_match(&wrong_arch));
    }

    #[test]
    fn contains_match_no_match_for_different_pkgname() {
        let (_tmp, root) = test_root("contains_match_no_match_for_different_pkgname");
        write_world(&root, "bpt\n");
        let world = World::from_root_path_ro(&root).unwrap();
        let pkgid = PkgId::new(
            PkgName::try_from("gcc").unwrap(),
            PkgVer::try_from("1.0.0").unwrap(),
            Arch::from_str("x86_64").unwrap(),
        );
        assert!(!world.contains_match(&pkgid));
    }

    #[test]
    fn get_match_returns_matching_partid() {
        let (_tmp, root) = test_root("get_match_returns_matching_partid");
        write_world(&root, "bpt@1.0.0:x86_64\n");
        let world = World::from_root_path_ro(&root).unwrap();
        let pkgid = PkgId::new(
            PkgName::try_from("bpt").unwrap(),
            PkgVer::try_from("1.0.0").unwrap(),
            Arch::from_str("x86_64").unwrap(),
        );
        let matched = world.get_match(&pkgid).unwrap();
        assert_eq!(*matched, PartId::from_str("bpt@1.0.0:x86_64").unwrap());
    }

    #[test]
    fn get_match_returns_none_when_absent() {
        let (_tmp, root) = test_root("get_match_returns_none_when_absent");
        write_world(&root, "bpt\n");
        let world = World::from_root_path_ro(&root).unwrap();
        let pkgid = PkgId::new(
            PkgName::try_from("gcc").unwrap(),
            PkgVer::try_from("1.0.0").unwrap(),
            Arch::from_str("x86_64").unwrap(),
        );
        assert!(world.get_match(&pkgid).is_none());
    }

    #[test]
    fn rw_creates_file_if_missing() {
        let (_tmp, root) = test_root("rw_creates_file_if_missing");
        let world_dir = root.as_path().join("etc/bpt");
        std::fs::create_dir_all(&world_dir).unwrap();
        let world = World::from_root_path_rw(&root).unwrap();
        assert!(world.entries().is_empty());
        assert!(world.writable);
    }

    #[test]
    fn rw_missing_parent_dirs_returns_empty() {
        let (_tmp, root) = test_root("rw_missing_parent_dirs_returns_empty");
        let world = World::from_root_path_rw(&root).unwrap();
        assert!(world.entries().is_empty());
        assert!(world.writable);
        assert!(root.as_path().join("etc/bpt").is_dir());
    }

    #[test]
    fn rw_reads_existing_entries() {
        let (_tmp, root) = test_root("rw_reads_existing_entries");
        write_world(&root, "bpt@1.0.0:x86_64\n");
        let world = World::from_root_path_rw(&root).unwrap();
        assert_eq!(world.entries().len(), 1);
        assert!(world.contains_entry(&PartId::from_str("bpt@1.0.0:x86_64").unwrap()));
    }

    #[test]
    fn save_writes_entries_to_disk() {
        let (_tmp, root) = test_root("save_writes_entries_to_disk");
        write_world(&root, "");
        let mut world = World::from_root_path_rw(&root).unwrap();
        world
            .entries_mut()
            .insert(PartId::from_str("bpt@1.0.0:x86_64").unwrap());
        world.save().unwrap();

        let reloaded = World::from_root_path_ro(&root).unwrap();
        assert!(reloaded.contains_entry(&PartId::from_str("bpt@1.0.0:x86_64").unwrap()));
    }

    #[test]
    fn save_creates_world_file_if_missing() {
        let (_tmp, root) = test_root("save_creates_world_file_if_missing");
        let mut world = World::from_root_path_rw(&root).unwrap();
        world
            .entries_mut()
            .insert(PartId::from_str("bpt@1.0.0:x86_64").unwrap());
        world.save().unwrap();

        let world_path = root.as_path().join(WORLD_PATH);
        assert!(world_path.exists());

        let reloaded = World::from_root_path_ro(&root).unwrap();
        assert!(reloaded.contains_entry(&PartId::from_str("bpt@1.0.0:x86_64").unwrap()));
    }

    #[test]
    fn save_removes_deleted_entries() {
        let (_tmp, root) = test_root("save_removes_deleted_entries");
        write_world(&root, "bpt@1.0.0:x86_64\ngcc@11.2.0:aarch64\n");
        let mut world = World::from_root_path_rw(&root).unwrap();
        world
            .entries_mut()
            .remove(&PartId::from_str("gcc@11.2.0:aarch64").unwrap());
        world.save().unwrap();

        let reloaded = World::from_root_path_ro(&root).unwrap();
        assert_eq!(reloaded.entries().len(), 1);
        assert!(reloaded.contains_entry(&PartId::from_str("bpt@1.0.0:x86_64").unwrap()));
        assert!(!reloaded.contains_entry(&PartId::from_str("gcc@11.2.0:aarch64").unwrap()));
    }

    #[test]
    fn save_preserves_comments() {
        let (_tmp, root) = test_root("save_preserves_comments");
        write_world(&root, "# keep this\nbpt@1.0.0:x86_64\n");
        let mut world = World::from_root_path_rw(&root).unwrap();
        world.save().unwrap();

        let content = std::fs::read_to_string(root.as_path().join(WORLD_PATH)).unwrap();
        assert!(content.contains("# keep this"));
        assert!(content.contains("bpt@1.0.0:x86_64"));
    }

    #[test]
    fn save_roundtrip_preserves_inline_comments() {
        let (_tmp, root) = test_root("save_roundtrip_preserves_inline_comments");
        write_world(&root, "bpt # explicitly requested\n");
        let mut world = World::from_root_path_rw(&root).unwrap();
        world.save().unwrap();

        let content = std::fs::read_to_string(root.as_path().join(WORLD_PATH)).unwrap();
        assert!(content.contains("bpt # explicitly requested"));
    }

    #[test]
    fn ro_rejects_invalid_partid() {
        let (_tmp, root) = test_root("ro_rejects_invalid_partid");
        write_world(&root, "bpt@1.0.0:badarch\n");
        assert!(World::from_root_path_ro(&root).is_err());
    }

    #[test]
    fn ro_deduplicates_entries() {
        let (_tmp, root) = test_root("ro_deduplicates_entries");
        write_world(&root, "bpt@1.0.0:x86_64\nbpt@1.0.0:x86_64\n");
        let world = World::from_root_path_ro(&root).unwrap();
        assert_eq!(world.entries().len(), 1);
    }
}
