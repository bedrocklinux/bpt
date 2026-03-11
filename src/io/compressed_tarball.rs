use crate::{constant::*, error::*, io::*, marshalling::*, metadata::*, str::*};
use camino::Utf8Path;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

type TarArchive<'a> = tar::Archive<
    CompressionDecoder<'a, std::io::Chain<std::io::Cursor<[u8; 4]>, &'a mut BoundedFile>>,
>;

pub struct CompressedTarball(BoundedFile);

impl CompressedTarball {
    pub fn from_bounded_file(file: BoundedFile) -> Self {
        Self(file)
    }

    pub fn from_dir(dir_path: &Utf8Path, file: File) -> Result<Self, Err> {
        let mut tar =
            tar::Builder::new(CompressionEncoder::new(file).loc(dir_path.join("<anon-bpt>"))?);

        // Skip ownership and mod/access time
        tar.mode(tar::HeaderMode::Deterministic);
        tar.follow_symlinks(false);
        tar.append_dir_all(".", dir_path)
            .map_err(AnonLocErr::BuildTarball)
            .loc(dir_path)?;
        tar.finish()
            .map_err(AnonLocErr::BuildTarball)
            .loc(dir_path)?;

        tar.into_inner()
            .map_err(AnonLocErr::BuildTarball)
            .loc(dir_path)?
            .finish()
            .and_then(BoundedFile::from_file)
            .map(Self::from_bounded_file)
            .loc(dir_path.join("<anon-bpt>"))
    }

    pub fn into_inner(self) -> BoundedFile {
        self.0
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.0.inner().link(path)
    }

    pub fn pkginfo(&mut self) -> Result<PkgInfo, AnonLocErr> {
        for entry in self.as_tar()?.entries().tbe()? {
            let mut entry = entry.tbe()?;
            let path = entry.path().tbe()?;
            if path.as_ref() != Path::new(TARBALL_PKGINFO_PATH) {
                continue;
            }

            // Cap read size to guard against OOM from a malformed tarball.  Package
            // metadata should be small; SMALL_FILE_MAX_SIZE is far more than legitimate
            // usage requires.
            let mut buf = Vec::new();
            entry
                .by_ref()
                .take(SMALL_FILE_MAX_SIZE as u64 + 1)
                .read_to_end(&mut buf)
                .tbe()?;
            if buf.len() > SMALL_FILE_MAX_SIZE {
                return Err(AnonLocErr::FieldInvalid(
                    "pkginfo",
                    "exceeded SMALL_FILE_MAX_SIZE".to_string(),
                ));
            }

            return PkgInfo::deserialize(&buf);
        }

        Err(AnonLocErr::FieldMissing("pkginfo"))
    }

    pub fn instfiles(&mut self) -> Result<Vec<InstFile>, AnonLocErr> {
        let mut entries = Vec::new();

        for entry in self.as_tar()?.entries().tbe()? {
            let mut entry = entry.tbe()?;
            let path = entry.path().tbe()?.strict_normalize().tbe()?;
            if path == Path::new(TARBALL_PKGINFO_PATH)
                || path == Path::new(TARBALL_ROOT_PATH)
                || path == Path::new("")
            {
                continue;
            }

            let header = entry.header();

            let entry = InstFile {
                path,
                mode: Mode::from_u32(header.mode().tbe()?),
                uid: Uid::from_u64(header.uid().tbe()?),
                gid: Gid::from_u64(header.gid().tbe()?),
                entry_type: match header.entry_type() {
                    tar::EntryType::Regular => {
                        // Stream file content through the hasher to avoid loading the
                        // entire file into memory.
                        let mut hasher = Sha256::new();
                        let mut buf = [0u8; 8192];
                        loop {
                            let n = entry.read(&mut buf).tbe()?;
                            if n == 0 {
                                break;
                            }
                            hasher.update(&buf[..n]);
                        }

                        let mut sha256 = [0u8; 256 / 8];
                        sha256.copy_from_slice(&hasher.finalize());
                        InstFileType::RegFile(RegFile::from_sha256(sha256))
                    }
                    tar::EntryType::Symlink => header
                        .link_name()
                        .tbe()?
                        .ok_or_else(|| {
                            AnonLocErr::ParseTarball(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "link entry without link name",
                            ))
                        })?
                        .into_pathbuf()
                        .map_err(AnonLocErr::ParseTarball)
                        .map(Symlink::from_pathbuf)
                        .map(InstFileType::Symlink)?,
                    tar::EntryType::Directory => InstFileType::Directory,
                    _ => Err(AnonLocErr::ParseTarball(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unsupported tar entry type",
                    )))?,
                },
            };

            entries.push(entry);
            if entries.len() > PACKAGE_FILE_COUNT {
                return Err(AnonLocErr::FieldInvalid(
                    "instfiles",
                    "too many tar entries".to_string(),
                ));
            }
        }

        Ok(entries)
    }

    pub fn as_tar(&mut self) -> Result<TarArchive<'_>, AnonLocErr> {
        self.0.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
        CompressionDecoder::new(&mut self.0).map(tar::Archive::new)
    }
}

trait ParseTarballErr<T> {
    fn tbe(self) -> Result<T, AnonLocErr>;
}

impl<T> ParseTarballErr<T> for std::io::Result<T> {
    fn tbe(self) -> Result<T, AnonLocErr> {
        self.map_err(AnonLocErr::ParseTarball)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshalling::{FieldList, FromFieldStr, Serialize};
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;
    use sha2::{Digest, Sha256};

    /// Build a minimal valid PkgInfo with known field values.
    fn make_test_pkginfo() -> PkgInfo {
        PkgInfo {
            pkgid: PkgId::new(
                PkgName::try_from("test-pkg").unwrap(),
                PkgVer::try_from("1.2.3").unwrap(),
                Arch::x86_64,
            ),
            pkgdesc: PkgDesc::from_field_str(FieldStr::try_from("A test package").unwrap())
                .unwrap(),
            homepage: crate::metadata::Homepage::from_field_str(
                FieldStr::try_from("https://example.com").unwrap(),
            )
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

    /// Serialize a PkgInfo to bytes.
    fn pkginfo_bytes(info: &PkgInfo) -> Vec<u8> {
        let mut buf = Vec::new();
        info.serialize(&mut buf).unwrap();
        buf
    }

    /// Entries to be added to a test tarball.
    enum TestEntry {
        Pkginfo(Vec<u8>),
        RegFile {
            path: &'static str,
            content: &'static [u8],
            mode: u32,
            uid: u64,
            gid: u64,
        },
        Dir {
            path: &'static str,
            mode: u32,
            uid: u64,
            gid: u64,
        },
        Symlink {
            path: &'static str,
            target: &'static str,
            mode: u32,
            uid: u64,
            gid: u64,
        },
        HardLink {
            path: &'static str,
            target: &'static str,
        },
    }

    /// Build a compressed tarball in a memfd from the given entries.
    fn make_tarball(entries: &[TestEntry]) -> CompressedTarball {
        let file = File::create_memfd(c"test_tarball", &[]).unwrap();
        let encoder = CompressionEncoder::new(file).unwrap();
        let mut tar = tar::Builder::new(encoder);

        for entry in entries {
            match entry {
                TestEntry::Pkginfo(data) => {
                    let mut header = tar::Header::new_gnu();
                    header.set_path(TARBALL_PKGINFO_PATH).unwrap();
                    header.set_size(data.len() as u64);
                    header.set_mode(0o644);
                    header.set_uid(0);
                    header.set_gid(0);
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_cksum();
                    tar.append(&header, &data[..]).unwrap();
                }
                TestEntry::RegFile {
                    path,
                    content,
                    mode,
                    uid,
                    gid,
                } => {
                    let mut header = tar::Header::new_gnu();
                    header.set_path(path).unwrap();
                    header.set_size(content.len() as u64);
                    header.set_mode(*mode);
                    header.set_uid(*uid);
                    header.set_gid(*gid);
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_cksum();
                    tar.append(&header, &content[..]).unwrap();
                }
                TestEntry::Dir {
                    path,
                    mode,
                    uid,
                    gid,
                } => {
                    let mut header = tar::Header::new_gnu();
                    header.set_path(path).unwrap();
                    header.set_size(0);
                    header.set_mode(*mode);
                    header.set_uid(*uid);
                    header.set_gid(*gid);
                    header.set_entry_type(tar::EntryType::Directory);
                    header.set_cksum();
                    tar.append(&header, std::io::empty()).unwrap();
                }
                TestEntry::Symlink {
                    path,
                    target,
                    mode,
                    uid,
                    gid,
                } => {
                    let mut header = tar::Header::new_gnu();
                    header.set_path(path).unwrap();
                    header.set_size(0);
                    header.set_mode(*mode);
                    header.set_uid(*uid);
                    header.set_gid(*gid);
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_link_name(target).unwrap();
                    header.set_cksum();
                    tar.append(&header, std::io::empty()).unwrap();
                }
                TestEntry::HardLink { path, target } => {
                    let mut header = tar::Header::new_gnu();
                    header.set_path(path).unwrap();
                    header.set_size(0);
                    header.set_mode(0o644);
                    header.set_uid(0);
                    header.set_gid(0);
                    header.set_entry_type(tar::EntryType::Link);
                    header.set_link_name(target).unwrap();
                    header.set_cksum();
                    tar.append(&header, std::io::empty()).unwrap();
                }
            }
        }

        tar.finish().unwrap();
        let encoder = tar.into_inner().unwrap();
        let file = encoder.finish().unwrap();
        let bounded_file = BoundedFile::from_file(file).unwrap();
        CompressedTarball::from_bounded_file(bounded_file)
    }

    fn sha256_of(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let mut sha = [0u8; 32];
        sha.copy_from_slice(&hasher.finalize());
        sha
    }

    // ------------------------------------------------------------------
    // pkginfo() tests
    // ------------------------------------------------------------------

    #[test]
    fn test_pkginfo_round_trip() {
        let info = make_test_pkginfo();
        let data = pkginfo_bytes(&info);
        let mut ct = make_tarball(&[TestEntry::Pkginfo(data)]);

        let extracted = ct.pkginfo().unwrap();
        assert_eq!(extracted.pkgid.pkgname, info.pkgid.pkgname);
        assert_eq!(extracted.pkgid.pkgver, info.pkgid.pkgver);
        assert_eq!(extracted.pkgid.arch, info.pkgid.arch);
        assert_eq!(extracted.pkgdesc, info.pkgdesc);
    }

    #[test]
    fn test_pkginfo_called_twice() {
        let info = make_test_pkginfo();
        let data = pkginfo_bytes(&info);
        let mut ct = make_tarball(&[TestEntry::Pkginfo(data)]);

        let first = ct.pkginfo().unwrap();
        let second = ct.pkginfo().unwrap();
        assert_eq!(first.pkgid, second.pkgid);
        assert_eq!(first.pkgdesc, second.pkgdesc);
    }

    #[test]
    fn test_pkginfo_missing() {
        let mut ct = make_tarball(&[TestEntry::RegFile {
            path: "usr/bin/hello",
            content: b"hello",
            mode: 0o755,
            uid: 0,
            gid: 0,
        }]);

        match ct.pkginfo() {
            Err(AnonLocErr::FieldMissing("pkginfo")) => {}
            other => panic!("expected FieldMissing(\"pkginfo\"), got: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // instfiles() tests
    // ------------------------------------------------------------------

    #[test]
    fn test_instfiles_regular_file() {
        let content = b"hello world";
        let expected_sha = sha256_of(content);

        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::RegFile {
                path: "usr/bin/hello",
                content,
                mode: 0o755,
                uid: 1000,
                gid: 1000,
            },
        ]);

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Utf8PathBuf::from("usr/bin/hello"));
        assert_eq!(files[0].mode, Mode::from_u32(0o755));
        assert_eq!(files[0].uid, Uid::from_u64(1000));
        assert_eq!(files[0].gid, Gid::from_u64(1000));
        match &files[0].entry_type {
            InstFileType::RegFile(rf) => {
                assert_eq!(*rf, RegFile::from_sha256(expected_sha));
            }
            other => panic!("expected RegFile, got: {other:?}"),
        }
    }

    #[test]
    fn test_from_dir_normalizes_ownership_to_root() {
        let dir = unit_test_tmp_dir(
            "compressed_tarball",
            "test_from_dir_normalizes_ownership_to_root",
        );
        let pkginfo_path = dir.join(TARBALL_PKGINFO_PATH);
        std::fs::write(&pkginfo_path, pkginfo_bytes(&make_test_pkginfo())).unwrap();
        let bin_dir = dir.join("usr/bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("hello"), b"hello world").unwrap();

        let file = File::create_anon(&dir).unwrap();
        let mut ct = CompressedTarball::from_dir(&dir, file).unwrap();
        let files = ct.instfiles().unwrap();
        let hello = files
            .iter()
            .find(|file| file.path == Utf8PathBuf::from("usr/bin/hello"))
            .unwrap();

        assert_eq!(hello.uid, Uid::from_u64(0));
        assert_eq!(hello.gid, Gid::from_u64(0));
    }

    #[test]
    fn test_instfiles_directory() {
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::Dir {
                path: "usr/lib/",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
        ]);

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Utf8PathBuf::from("usr/lib/"));
        assert!(matches!(files[0].entry_type, InstFileType::Directory));
    }

    #[test]
    fn test_instfiles_symlink() {
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::Symlink {
                path: "usr/lib/libfoo.so",
                target: "libfoo.so.1",
                mode: 0o777,
                uid: 0,
                gid: 0,
            },
        ]);

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Utf8PathBuf::from("usr/lib/libfoo.so"));
        match &files[0].entry_type {
            InstFileType::Symlink(s) => {
                assert_eq!(*s, Symlink::from_pathbuf(Utf8PathBuf::from("libfoo.so.1")));
            }
            other => panic!("expected Symlink, got: {other:?}"),
        }
    }

    #[test]
    fn test_instfiles_mixed_entries() {
        let content = b"binary";

        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::Dir {
                path: "usr/",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
            TestEntry::Dir {
                path: "usr/bin/",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
            TestEntry::RegFile {
                path: "usr/bin/app",
                content,
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
            TestEntry::Symlink {
                path: "usr/bin/link",
                target: "app",
                mode: 0o777,
                uid: 0,
                gid: 0,
            },
        ]);

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 4);

        // Directories
        assert!(matches!(files[0].entry_type, InstFileType::Directory));
        assert!(matches!(files[1].entry_type, InstFileType::Directory));
        // Regular file
        assert!(matches!(files[2].entry_type, InstFileType::RegFile(_)));
        // Symlink
        assert!(matches!(files[3].entry_type, InstFileType::Symlink(_)));
    }

    #[test]
    fn test_instfiles_skips_pkginfo_and_root() {
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::Dir {
                path: "./",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
            TestEntry::RegFile {
                path: "usr/bin/hello",
                content: b"hello",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
        ]);

        let files = ct.instfiles().unwrap();
        // .pkginfo and ./ should be filtered; only the regular file should remain
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Utf8PathBuf::from("usr/bin/hello"));
    }

    #[test]
    fn test_instfiles_normalizes_paths() {
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::RegFile {
                path: "./usr/bin/hello",
                content: b"hello",
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
        ]);

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 1);
        // Leading ./ should be stripped by normalize()
        assert_eq!(files[0].path, Utf8PathBuf::from("usr/bin/hello"));
    }

    #[test]
    fn test_instfiles_unsupported_entry_type() {
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::HardLink {
                path: "usr/bin/link",
                target: "usr/bin/app",
            },
        ]);

        match ct.instfiles() {
            Err(AnonLocErr::ParseTarball(e)) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
                assert!(e.to_string().contains("unsupported tar entry type"));
            }
            other => panic!("expected ParseTarball error, got: {other:?}"),
        }
    }

    #[test]
    fn test_instfiles_after_pkginfo() {
        let content = b"data";
        let mut ct = make_tarball(&[
            TestEntry::Pkginfo(pkginfo_bytes(&make_test_pkginfo())),
            TestEntry::RegFile {
                path: "etc/config",
                content,
                mode: 0o644,
                uid: 0,
                gid: 0,
            },
        ]);

        // Call pkginfo first, then instfiles - both should succeed due to as_tar() rewind
        let info = ct.pkginfo().unwrap();
        assert_eq!(info.pkgid.pkgname.as_str(), "test-pkg");

        let files = ct.instfiles().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Utf8PathBuf::from("etc/config"));
    }

    // Cannot test: the tar crate's `set_path` rejects paths containing `..`, so we cannot
    // construct a test tarball with path traversal entries.  The `strict_normalize` rejection
    // is tested directly in `str::pathbuf::tests::test_strict_normalize_rejects_dotdot`.
    //
    // #[test]
    // fn test_instfiles_rejects_dotdot() { ... }

    // Cannot test: the tar crate's `set_path` strips non-leading `.` components, and
    // `std::path::Component` also strips them during iteration, so the defensive check in
    // `strict_normalize` cannot trigger from tarball input.
    //
    // #[test]
    // fn test_instfiles_rejects_non_leading_dot() { ... }
}
