use crate::{constant::*, error::*, io::*, location::RootDir, marshalling::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{cmp::Ordering, fs::File, io::Write};

/// A file describing an installed package.
///
/// Referenced against actual installed files to check installed package status.
///
/// Used to know which files to remove when uninstalling a package.
///
/// Filenames typically end in `.instpkg`.
pub struct InstPkg {
    pkginfo: PkgInfo,
    entries: Vec<InstFile>,
    file: File,
    // Track file path so we can remove file when uninstalling.
    path: Utf8PathBuf,
}

pub enum InstPkgCheckIssue {
    Error(String),
    BackupDiff(String),
}

impl InstPkgCheckIssue {}

impl MagicNumber for InstPkg {
    const DESCRIPTION: &'static str = "bpt installed package metadata";
    const MAGIC: &'static [u8] = INSTPKG_MAGIC;
}

macro_rules! extract {
    ($ty:ty, $field:expr, $path:expr) => {
        $field.ok_or_else(|| Err::FieldMissing($path.to_string(), <$ty>::NAME))
    };
}

macro_rules! serialize {
    ($value:expr, $file:expr, $loc:expr) => {
        $value.serialize($file).loc(&*$loc)
    };
}

/// Compare instfile entries for sorting before serialization.
///
/// Produces a depth-first traversal order where files at each directory level sort before
/// subdirectories.  The serializer depends on this: it tracks a directory context and requires
/// that all files in a directory are emitted before descending into subdirectories.
///
/// The comparison walks path components in lockstep, assigning rank 0 to files/symlinks at their
/// final component and rank 1 to directories and intermediate components.  Comparing (rank, name)
/// at each level ensures transitivity.
fn cmp_instfile_entries(a: &InstFile, b: &InstFile) -> Ordering {
    // Degenerate case: same path, directory sorts first
    if a.path == b.path {
        let a_is_dir = matches!(a.entry_type, InstFileType::Directory);
        let b_is_dir = matches!(b.entry_type, InstFileType::Directory);
        return b_is_dir.cmp(&a_is_dir);
    }

    let a_n = a.path.as_str().split('/').count();
    let b_n = b.path.as_str().split('/').count();

    for (i, (ac, bc)) in a
        .path
        .as_str()
        .split('/')
        .zip(b.path.as_str().split('/'))
        .enumerate()
    {
        // Files/symlinks at their final component get rank 0 (sort first);
        // directories and intermediate components get rank 1.
        let a_rank: u8 = if i + 1 == a_n && !matches!(a.entry_type, InstFileType::Directory) {
            0
        } else {
            1
        };
        let b_rank: u8 = if i + 1 == b_n && !matches!(b.entry_type, InstFileType::Directory) {
            0
        } else {
            1
        };

        match a_rank.cmp(&b_rank) {
            Ordering::Equal => match ac.cmp(bc) {
                Ordering::Equal => continue,
                other => return other,
            },
            other => return other,
        }
    }

    // One path is a prefix of the other; shorter (parent) comes first
    a_n.cmp(&b_n)
}

impl InstPkg {
    pub fn from_path(path: Utf8PathBuf) -> Result<Self, Err> {
        let mut file = File::open_ro(&path)?
            .verify_and_strip_magic::<Self>()
            .loc(&path)?;
        let bytes = file.read_small_file_bytes().loc(&path)?;
        let mut block_iter = bytes.as_block_iter();

        let pkginfo_block = block_iter
            .next()
            .ok_or_else(|| Err::FieldMissing(path.to_string(), "pkginfo"))?;
        let pkginfo = PkgInfo::deserialize(pkginfo_block).loc(&path)?;

        let entry_block = block_iter
            .next()
            .ok_or_else(|| Err::FieldMissing(path.to_string(), "entries"))?;
        let mut entries = Vec::new();

        // Sequential files often share the same metadata, and so we re-use the metadata of a
        // preceding file and until it is overwritten.
        let mut mode = None;
        let mut uid = None;
        let mut gid = None;
        let mut filename = None;
        let mut dir = Dir::empty();

        // Strip the trailing null terminator left by the last serialized field.
        // After stripping, any remaining empty slice from split indicates consecutive
        // nulls (corruption), not protocol structure.
        let entry_data = entry_block.strip_suffix(b"\0").unwrap_or(entry_block);

        for field in entry_data.split(|&b| b == b'\0') {
            if field.is_empty() {
                return Err(Err::UnexpectedData(path.to_string()));
            }

            let entry = match field[0] {
                Mode::KEY => {
                    mode = Some(Mode::deserialize(field).loc(&path)?);
                    continue;
                }
                Uid::KEY => {
                    uid = Some(Uid::deserialize(field).loc(&path)?);
                    continue;
                }
                Gid::KEY => {
                    gid = Some(Gid::deserialize(field).loc(&path)?);
                    continue;
                }
                Filename::KEY => {
                    filename = Some(Filename::deserialize(field).loc(&path)?);
                    continue;
                }
                Dir::KEY => {
                    let dir_path = Dir::deserialize(field).loc(&path)?;
                    dir = dir_path.clone();
                    InstFile {
                        mode: extract!(Mode, mode, path)?,
                        uid: extract!(Uid, uid, path)?,
                        gid: extract!(Gid, gid, path)?,
                        path: dir_path.into_pathbuf(),
                        entry_type: InstFileType::Directory,
                    }
                }
                Subdir::KEY => {
                    dir.push(&Subdir::deserialize(field).loc(&path)?.into_pathbuf());
                    InstFile {
                        mode: extract!(Mode, mode, path)?,
                        uid: extract!(Uid, uid, path)?,
                        gid: extract!(Gid, gid, path)?,
                        path: dir.as_path().to_owned(),
                        entry_type: InstFileType::Directory,
                    }
                }
                RegFile::KEY => {
                    let file_path = dir
                        .as_path()
                        .join(extract!(Filename, filename.take(), path)?.into_pathbuf());
                    InstFile {
                        mode: extract!(Mode, mode, path)?,
                        uid: extract!(Uid, uid, path)?,
                        gid: extract!(Gid, gid, path)?,
                        path: file_path,
                        entry_type: InstFileType::RegFile(RegFile::deserialize(field).loc(&path)?),
                    }
                }
                Symlink::KEY => {
                    let file_path = dir
                        .as_path()
                        .join(extract!(Filename, filename.take(), path)?.into_pathbuf());
                    InstFile {
                        mode: extract!(Mode, mode, path)?,
                        uid: extract!(Uid, uid, path)?,
                        gid: extract!(Gid, gid, path)?,
                        path: file_path,
                        entry_type: InstFileType::Symlink(Symlink::deserialize(field).loc(&path)?),
                    }
                }
                _ => {
                    return Err(Err::UnexpectedData(path.to_string()));
                }
            };
            entries.push(entry);
        }

        Ok(Self {
            pkginfo,
            entries,
            file: file.into_inner(),
            path,
        })
    }

    pub fn from_pkginfo_and_entries(
        pkginfo: PkgInfo,
        mut entries: Vec<InstFile>,
        out_dir: &Utf8Path,
    ) -> Result<Self, Err> {
        let mut file = File::create_anon(out_dir)?;
        let err_loc = out_dir.join("<anon-instpkg>");

        file.write_all(Self::MAGIC)
            .map_err(AnonLocErr::Write)
            .loc(&err_loc)?;
        pkginfo.serialize(&mut file).loc(&err_loc)?;
        file.write_all(b"\0")
            .map_err(AnonLocErr::Write)
            .loc(&err_loc)?;

        // Ensure parent directories are serialized before child file entries that depend on the
        // current directory context.
        entries.sort_by(cmp_instfile_entries);

        // Files often share the same metadata, so we cache the metadata of the previous file and
        // re-use it for multiple files if not explicitly overwritten.
        let mut mode = None;
        let mut uid = None;
        let mut gid = None;
        let mut dir = Dir::empty();

        for entry in &entries {
            if Some(&entry.mode) != mode.as_ref() {
                mode = Some(entry.mode);
                serialize!(entry.mode, &mut file, err_loc)?;
            }
            if Some(&entry.uid) != uid.as_ref() {
                uid = Some(entry.uid);
                serialize!(entry.uid, &mut file, err_loc)?;
            }
            if Some(&entry.gid) != gid.as_ref() {
                gid = Some(entry.gid);
                serialize!(entry.gid, &mut file, err_loc)?;
            }
            let subpath = entry.path.strip_prefix(dir.as_path()).ok();

            match &entry.entry_type {
                InstFileType::Directory => match subpath {
                    Some(subpath) => {
                        serialize!(Subdir::from_pathbuf(subpath.to_owned()), &mut file, err_loc)?;
                        dir.push(subpath);
                    }
                    None => {
                        serialize!(Dir::from_pathbuf(entry.path.to_owned()), &mut file, err_loc)?;
                        dir = Dir::from_pathbuf(entry.path.to_owned());
                    }
                },
                InstFileType::RegFile(regfile) => {
                    let filename = subpath
                        .ok_or_else(|| Err::UnexpectedData(err_loc.to_string()))
                        .map(Filename::from_path)?;
                    serialize!(filename, &mut file, err_loc)?;
                    serialize!(regfile, &mut file, err_loc)?;
                }
                InstFileType::Symlink(symlink) => {
                    let filename = subpath
                        .ok_or_else(|| Err::UnexpectedData(err_loc.to_string()))
                        .map(Filename::from_path)?;
                    serialize!(filename, &mut file, err_loc)?;
                    serialize!(symlink, &mut file, err_loc)?;
                }
            }
        }

        let mut result = Self {
            pkginfo,
            entries,
            file,
            path: Utf8PathBuf::new(),
        };
        result.path = out_dir.join(result.canonical_filename());
        Ok(result)
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.file.link(path)
    }

    pub fn pkginfo(&self) -> &PkgInfo {
        &self.pkginfo
    }

    pub fn pkgid(&self) -> &PkgId {
        self.pkginfo.pkgid()
    }

    pub fn canonical_filename(&self) -> Utf8PathBuf {
        let pkgid = self.pkginfo.pkgid();
        format!("{}@{}:{}.instpkg", pkgid.pkgname, pkgid.pkgver, pkgid.arch).into()
    }

    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }

    pub fn paths(&self) -> impl Iterator<Item = &Utf8Path> {
        self.entries.iter().map(|entry| entry.path.as_path())
    }

    pub fn entries(&self) -> &[InstFile] {
        &self.entries
    }

    pub fn uninstall(&self, root: &RootDir, purge: bool, forget: bool) -> Result<(), Err> {
        if !forget {
            // Reverse sort to ensure directories are removed after their contents.
            let mut entries = self.entries.iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| b.path.cmp(&a.path));

            for entry in entries {
                // Retain modified backup files unless purging.
                if !purge
                    && self.is_backup_path(&entry.path)
                    && entry.is_content_modified(root.as_path())?
                {
                    continue;
                }
                entry.remove(root.as_path())?;
            }
        }

        std::fs::remove_file(&self.path).map_err(|e| Err::Remove(self.path.to_string(), e))
    }

    fn is_backup_path(&self, path: &Utf8Path) -> bool {
        self.pkginfo.backup.iter().any(|b| b.as_path() == path)
    }

    pub fn check(&self, root: &Utf8Path) -> Result<Vec<InstPkgCheckIssue>, Err> {
        let mut issues = Vec::new();
        for entry in &self.entries {
            for issue in entry.check(root)? {
                if self.is_backup_path(&entry.path) && issue.is_content_difference {
                    issues.push(InstPkgCheckIssue::BackupDiff(issue.message));
                } else {
                    issues.push(InstPkgCheckIssue::Error(issue.message));
                }
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshalling::{FieldStr, FromFieldStr};
    use crate::testutil::unit_test_tmp_dir;
    use camino::{Utf8Path, Utf8PathBuf};
    use sha2::{Digest, Sha256};
    use std::io::Write;

    fn test_root(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("instpkg", name)
    }

    #[test]
    fn from_path_rejects_oversized_instpkg() {
        let root = test_root("from_path_rejects_oversized_instpkg");
        let path = root.join("too-big.instpkg");

        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(INSTPKG_MAGIC).unwrap();
        let payload = vec![b'x'; SMALL_FILE_MAX_SIZE + 1];
        file.write_all(&payload).unwrap();
        drop(file);

        match InstPkg::from_path(path.clone()) {
            Err(Err::FileTooLarge(loc, max)) => {
                assert_eq!(loc, path.to_string());
                assert_eq!(max, SMALL_FILE_MAX_SIZE);
            }
            Err(other) => panic!("expected FileTooLarge for oversized instpkg, got {other}"),
            Ok(_) => panic!("expected FileTooLarge for oversized instpkg, got Ok(_)"),
        }
    }

    #[test]
    fn uninstall_removes_empty_parent_directories() {
        let root = test_root("uninstall_removes_empty_parent_directories");
        let root_dir = RootDir::from_path(root.as_path());
        let pkginfo = make_test_pkginfo();
        let entries = vec![
            make_dir("usr"),
            make_dir("usr/share"),
            make_dir("usr/share/zsh"),
            make_dir("usr/share/zsh/site-functions"),
            make_reg("usr/share/zsh/site-functions/_test", b"completion"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();
        std::fs::create_dir_all(root.join("usr/share/zsh/site-functions")).unwrap();
        std::fs::write(
            root.join("usr/share/zsh/site-functions/_test"),
            b"completion",
        )
        .unwrap();

        instpkg.uninstall(&root_dir, false, false).unwrap();

        assert!(!root.join("usr/share/zsh/site-functions/_test").exists());
        assert!(!root.join("usr/share/zsh/site-functions").exists());
        assert!(!root.join("usr/share/zsh").exists());
        assert!(!root.join("usr/share").exists());
        assert!(!root.join("usr").exists());
        assert!(!instpkg.path().exists());
    }

    fn make_test_pkginfo() -> PkgInfo {
        PkgInfo {
            pkgid: PkgId::new(
                PkgName::try_from("test-pkg").unwrap(),
                PkgVer::try_from("1.2.3").unwrap(),
                Arch::x86_64,
            ),
            pkgdesc: PkgDesc::from_field_str(FieldStr::try_from("A test package").unwrap())
                .unwrap(),
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

    fn sha256_of(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        out
    }

    fn make_dir(path: &str) -> InstFile {
        InstFile {
            mode: Mode::from_u32(0o755),
            uid: Uid::from_u64(0),
            gid: Gid::from_u64(0),
            path: Utf8PathBuf::from(path),
            entry_type: InstFileType::Directory,
        }
    }

    fn make_reg(path: &str, data: &[u8]) -> InstFile {
        InstFile {
            mode: Mode::from_u32(0o644),
            uid: Uid::from_u64(0),
            gid: Gid::from_u64(0),
            path: Utf8PathBuf::from(path),
            entry_type: InstFileType::RegFile(RegFile::from_sha256(sha256_of(data))),
        }
    }

    fn make_symlink(path: &str, target: &str) -> InstFile {
        InstFile {
            mode: Mode::from_u32(0o777),
            uid: Uid::from_u64(0),
            gid: Gid::from_u64(0),
            path: Utf8PathBuf::from(path),
            entry_type: InstFileType::Symlink(Symlink::from_pathbuf(target.into())),
        }
    }

    #[test]
    fn cmp_orders_parent_directory_before_child_file() {
        let parent = make_dir("etc");
        let child = make_reg("etc/config", b"config");
        assert_eq!(cmp_instfile_entries(&parent, &child), Ordering::Less);
        assert_eq!(cmp_instfile_entries(&child, &parent), Ordering::Greater);
    }

    #[test]
    fn cmp_orders_same_path_directory_before_non_directory() {
        let dir = make_dir("etc");
        let reg = make_reg("etc", b"file");
        let sym = make_symlink("etc", "/tmp/target");
        assert_eq!(cmp_instfile_entries(&dir, &reg), Ordering::Less);
        assert_eq!(cmp_instfile_entries(&dir, &sym), Ordering::Less);
    }

    #[test]
    fn cmp_never_treats_parent_and_child_as_equal() {
        let parent = make_dir("usr/lib");
        let child = make_symlink("usr/lib/libfoo.so", "libfoo.so.1");
        assert_ne!(cmp_instfile_entries(&parent, &child), Ordering::Equal);
        assert_ne!(cmp_instfile_entries(&child, &parent), Ordering::Equal);
    }

    #[test]
    fn from_pkginfo_and_entries_accepts_shuffled_parent_child_order() {
        let root = test_root("from_pkginfo_and_entries_accepts_shuffled_parent_child_order");
        let pkginfo = make_test_pkginfo();
        let entries = vec![
            make_reg("etc/app/config.toml", b"port=8080"),
            make_dir("etc"),
            make_symlink("etc/app/link", "../config.toml"),
            make_dir("etc/app"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();

        // Ensure serialized file can be parsed again after linking to canonical path.
        instpkg.link(instpkg.path()).unwrap();
        let reloaded = InstPkg::from_path(instpkg.path().to_path_buf()).unwrap();
        let reloaded_paths = reloaded.paths().collect::<Vec<&Utf8Path>>();
        assert_eq!(
            reloaded_paths,
            vec![
                Utf8Path::new("etc"),
                Utf8Path::new("etc/app"),
                Utf8Path::new("etc/app/config.toml"),
                Utf8Path::new("etc/app/link"),
            ]
        );
    }

    #[test]
    fn from_pkginfo_and_entries_accepts_deeply_shuffled_tree() {
        let root = test_root("from_pkginfo_and_entries_accepts_deeply_shuffled_tree");
        let pkginfo = make_test_pkginfo();
        let entries = vec![
            make_reg("usr/lib/foo/plugins/a.so", b"a"),
            make_dir("usr"),
            make_reg("usr/bin/foo", b"#!/bin/sh"),
            make_dir("usr/lib/foo"),
            make_dir("usr/bin"),
            make_dir("usr/lib"),
            make_dir("usr/lib/foo/plugins"),
            make_symlink("usr/lib/foo/current", "plugins/a.so"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();
        let reloaded = InstPkg::from_path(instpkg.path().to_path_buf()).unwrap();
        let reloaded_paths = reloaded.paths().collect::<Vec<&Utf8Path>>();

        assert_eq!(
            reloaded_paths,
            vec![
                Utf8Path::new("usr"),
                Utf8Path::new("usr/bin"),
                Utf8Path::new("usr/bin/foo"),
                Utf8Path::new("usr/lib"),
                Utf8Path::new("usr/lib/foo"),
                Utf8Path::new("usr/lib/foo/current"),
                Utf8Path::new("usr/lib/foo/plugins"),
                Utf8Path::new("usr/lib/foo/plugins/a.so"),
            ]
        );
    }

    #[test]
    fn from_pkginfo_and_entries_accepts_sibling_file_after_subdir() {
        let root = test_root("from_pkginfo_and_entries_accepts_sibling_file_after_subdir");
        let pkginfo = make_test_pkginfo();
        let entries = vec![
            make_dir("etc"),
            make_dir("etc/ssl"),
            make_reg("etc/z.conf", b"zone=UTC"),
            make_reg("etc/ssl/cert.pem", b"-----BEGIN CERTIFICATE-----"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();
        let reloaded = InstPkg::from_path(instpkg.path().to_path_buf()).unwrap();
        let reloaded_paths = reloaded.paths().collect::<Vec<&Utf8Path>>();

        assert_eq!(
            reloaded_paths,
            vec![
                Utf8Path::new("etc"),
                Utf8Path::new("etc/z.conf"),
                Utf8Path::new("etc/ssl"),
                Utf8Path::new("etc/ssl/cert.pem"),
            ]
        );
    }

    #[test]
    fn from_pkginfo_and_entries_accepts_root_relative_items() {
        let root = test_root("from_pkginfo_and_entries_accepts_root_relative_items");
        let pkginfo = make_test_pkginfo();
        let entries = vec![
            make_reg("README", b"hello"),
            make_symlink("latest", "README"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();
        let reloaded = InstPkg::from_path(instpkg.path().to_path_buf()).unwrap();
        let reloaded_paths = reloaded.paths().collect::<Vec<&Utf8Path>>();

        assert_eq!(
            reloaded_paths,
            vec![Utf8Path::new("README"), Utf8Path::new("latest"),]
        );
    }

    #[test]
    fn from_path_rejects_consecutive_nulls() {
        let root = test_root("from_path_rejects_consecutive_nulls");
        let pkginfo = make_test_pkginfo();
        let entries = vec![make_dir("etc"), make_reg("etc/config", b"data")];

        // Build a valid instpkg and link it to disk.
        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();

        // Read the valid file, inject a null byte at the start of the entries block.
        // This produces a leading empty field when the block is split on nulls.
        //
        // Note: injecting \0 between existing field separators would create \0\0, which
        // BlockIter interprets as a block boundary rather than consecutive nulls within
        // a block.  Inserting at the entries block start avoids that ambiguity.
        let valid_bytes = std::fs::read(instpkg.path()).unwrap();
        let mut corrupted = Vec::with_capacity(valid_bytes.len() + 1);

        let block_sep = valid_bytes
            .windows(2)
            .position(|w| w == [0, 0])
            .expect("instpkg should contain a block separator");
        let entries_start = block_sep + 2;

        corrupted.extend_from_slice(&valid_bytes[..entries_start]);
        corrupted.push(0); // extra null at start of entries block
        corrupted.extend_from_slice(&valid_bytes[entries_start..]);

        let corrupted_path = root.join("corrupted.instpkg");
        std::fs::write(&corrupted_path, &corrupted).unwrap();

        match InstPkg::from_path(corrupted_path.clone()) {
            Err(Err::UnexpectedData(loc)) => {
                assert_eq!(loc, corrupted_path.to_string());
            }
            Err(other) => panic!("expected UnexpectedData for consecutive nulls, got {other}"),
            Ok(_) => panic!("expected UnexpectedData for consecutive nulls, got Ok(_)"),
        }
    }

    #[test]
    fn cmp_is_transitive_for_files_alongside_subdirs() {
        // This triple was intransitive with the old comparator:
        // A < B (files before dirs at same parent), B < C (prefix), but A > C (path compare)
        let a = make_reg("usr/x86_64/include/time.h", b"");
        let b = make_dir("usr/x86_64/include/sys");
        let c = make_reg("usr/x86_64/include/sys/socket.h", b"");

        let ab = cmp_instfile_entries(&a, &b);
        let bc = cmp_instfile_entries(&b, &c);
        let ac = cmp_instfile_entries(&a, &c);

        assert_eq!(
            ab,
            Ordering::Less,
            "file time.h should sort before dir sys/"
        );
        assert_eq!(bc, Ordering::Less, "dir sys/ should sort before its child");
        assert_eq!(ac, Ordering::Less, "transitivity: a < b < c implies a < c");
    }

    #[test]
    fn from_pkginfo_and_entries_handles_files_alongside_deep_subdirs() {
        let root = test_root("from_pkginfo_and_entries_handles_files_alongside_deep_subdirs");
        let pkginfo = make_test_pkginfo();

        // Mimics a musl-like layout: files at a directory level alongside subdirectories
        let entries = vec![
            make_dir("usr"),
            make_dir("usr/x86_64"),
            make_dir("usr/x86_64/include"),
            make_reg("usr/x86_64/include/stdlib.h", b""),
            make_reg("usr/x86_64/include/string.h", b""),
            make_reg("usr/x86_64/include/time.h", b""),
            make_dir("usr/x86_64/include/arpa"),
            make_reg("usr/x86_64/include/arpa/inet.h", b""),
            make_dir("usr/x86_64/include/sys"),
            make_reg("usr/x86_64/include/sys/socket.h", b""),
            make_reg("usr/x86_64/include/sys/types.h", b""),
            make_dir("usr/x86_64/lib"),
            make_reg("usr/x86_64/lib/libc.a", b""),
            make_symlink("usr/x86_64/lib/libc.so", "libc.a"),
        ];

        let instpkg = InstPkg::from_pkginfo_and_entries(pkginfo, entries, root.as_path()).unwrap();
        instpkg.link(instpkg.path()).unwrap();
        let reloaded = InstPkg::from_path(instpkg.path().to_path_buf()).unwrap();
        let reloaded_paths = reloaded.paths().collect::<Vec<&Utf8Path>>();

        assert_eq!(
            reloaded_paths,
            vec![
                Utf8Path::new("usr"),
                Utf8Path::new("usr/x86_64"),
                Utf8Path::new("usr/x86_64/include"),
                Utf8Path::new("usr/x86_64/include/stdlib.h"),
                Utf8Path::new("usr/x86_64/include/string.h"),
                Utf8Path::new("usr/x86_64/include/time.h"),
                Utf8Path::new("usr/x86_64/include/arpa"),
                Utf8Path::new("usr/x86_64/include/arpa/inet.h"),
                Utf8Path::new("usr/x86_64/include/sys"),
                Utf8Path::new("usr/x86_64/include/sys/socket.h"),
                Utf8Path::new("usr/x86_64/include/sys/types.h"),
                Utf8Path::new("usr/x86_64/lib"),
                Utf8Path::new("usr/x86_64/lib/libc.a"),
                Utf8Path::new("usr/x86_64/lib/libc.so"),
            ]
        );
    }
}
