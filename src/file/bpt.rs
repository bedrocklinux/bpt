use crate::{
    constant::*, error::*, file::*, io::*, location::RootDir, marshalling::*, metadata::*, str::*,
};
use camino::{Utf8Path, Utf8PathBuf};
use std::{collections::HashSet, fs::File, io::Write, path::Path};

/// Binary package
///
/// Filenames typically end in `.bpt`.
pub struct Bpt {
    pkginfo: PkgInfo,
    tarball: CompressedTarball,
    // Reading data from the embedded tarball requires mutation to move the cursor within the file.
    // To avoid this requirement, commonly requested data is preemptively extracted when creating
    // the type.
    instfiles: Vec<InstFile>,
    filepaths: Vec<Utf8PathBuf>,
}

impl MagicNumber for Bpt {
    const DESCRIPTION: &'static str = "bpt binary package";
    const MAGIC: &'static [u8] = BPT_MAGIC;
}

impl Bpt {
    pub fn from_file(file: File, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr> {
        let file = file
            .verify_and_strip_sig(pubkeys)?
            .verify_and_strip_magic::<Self>()?;

        let mut tarball = CompressedTarball::from_bounded_file(file);
        let pkginfo = tarball.pkginfo()?;
        let instfiles = tarball.instfiles()?;
        let mut filepaths = instfiles.iter().map(|f| f.path.clone()).collect::<Vec<_>>();
        filepaths.sort_unstable();

        Ok(Self {
            pkginfo,
            tarball,
            instfiles,
            filepaths,
        })
    }

    pub fn from_dir(in_dir: &Utf8Path, out_dir: &Utf8Path, privkey: &PrivKey) -> Result<Self, Err> {
        let mut file = File::create_anon(out_dir)?;

        file.write_all(Self::MAGIC)
            .map_err(AnonLocErr::Write)
            .loc(out_dir.join("<anon-bpt>"))?;

        // This can take a while.  Let the user know what's happening.
        print!("Compressing package... ");
        let mut file = CompressedTarball::from_dir(in_dir, file)?
            .into_inner()
            .into_inner();
        println!("done");

        file.sign(privkey).loc(out_dir.join("<anon-bpt>"))?;

        Self::from_file(file, &PublicKeys::from_skipping_verification()).loc(in_dir)
    }

    pub fn pkginfo(&self) -> &PkgInfo {
        &self.pkginfo
    }

    pub fn pkgid(&self) -> &PkgId {
        self.pkginfo.pkgid()
    }

    pub fn filepaths(&self) -> &[Utf8PathBuf] {
        &self.filepaths
    }

    pub fn instfiles(&self) -> &[InstFile] {
        &self.instfiles
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.tarball.link(path)
    }

    pub fn install(
        &mut self,
        root: &RootDir,
        instpkg_dir: &Utf8Path,
        bptnew: &mut Vec<Utf8PathBuf>,
    ) -> Result<(), Err> {
        let instpkg = InstPkg::from_pkginfo_and_entries(
            self.pkginfo.clone(),
            self.instfiles.clone(),
            instpkg_dir,
        )?;
        // A file which represents an installed package.  Used by things like `bpt check` and
        // `bpt remove`.
        //
        // Install this first so that we're able to `bpt remove` a broke package install if a
        // following step errors out.
        instpkg.link(instpkg.path())?;
        self.unpack_payload(root, bptnew)
    }

    pub fn upgrade(
        &mut self,
        old: &InstPkg,
        root: &RootDir,
        instpkg_dir: &Utf8Path,
        bptnew: &mut Vec<Utf8PathBuf>,
    ) -> Result<(), Err> {
        let new_instpkg = InstPkg::from_pkginfo_and_entries(
            self.pkginfo.clone(),
            self.instfiles.clone(),
            instpkg_dir,
        )?;
        let new_paths = self
            .instfiles
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<HashSet<_>>();

        self.unpack_payload(root, bptnew)?;

        // Remove files that belonged to the old package but are absent from the new one.
        let mut old_only_entries = old
            .entries()
            .iter()
            .filter(|entry| !new_paths.contains(&entry.path))
            .collect::<Vec<_>>();
        old_only_entries.sort_by(|a, b| b.path.cmp(&a.path));

        for entry in old_only_entries {
            if old
                .pkginfo()
                .backup
                .iter()
                .any(|backup| backup.as_path() == entry.path)
                && entry.is_content_modified(root.as_path())?
            {
                continue;
            }
            entry.remove(root.as_path())?;
        }

        if old.path() == new_instpkg.path() {
            std::fs::remove_file(old.path()).map_err(|e| Err::Remove(old.path().to_string(), e))?;
        }
        new_instpkg.link(new_instpkg.path())?;
        if old.path() != new_instpkg.path() {
            std::fs::remove_file(old.path()).map_err(|e| Err::Remove(old.path().to_string(), e))?;
        }

        Ok(())
    }

    fn unpack_payload(&mut self, root: &RootDir, bptnew: &mut Vec<Utf8PathBuf>) -> Result<(), Err> {
        let pkgid = self.pkgid().clone();
        let backup_paths: Vec<Utf8PathBuf> = self
            .pkginfo
            .backup
            .iter()
            .map(|b| b.as_path().to_owned())
            .collect();

        for entry in self
            .tarball
            .as_tar()
            .loc("<anon-bpt>")?
            .entries()
            .map_err(|e| Err::ParseTarball(pkgid.to_string(), e))?
        {
            let mut entry = entry.map_err(|e| Err::ParseTarball(pkgid.to_string(), e))?;
            let path = entry
                .path()
                .map_err(|e| Err::ParseTarball(pkgid.to_string(), e))?;
            if path == Path::new(TARBALL_PKGINFO_PATH) || path == Path::new(TARBALL_ROOT_PATH) {
                continue;
            }

            let normalized = path
                .strict_normalize()
                .map_err(|e| Err::ParseTarball(pkgid.to_string(), e))?;

            // Avoid clobbering user-modified backup files (e.g. config files).  If the file is
            // listed in backup and already exists on disk, install the package's version as
            // <path>.bptnew and let the caller notify the user to merge.
            if backup_paths.contains(&normalized) && root.as_path().join(&normalized).exists() {
                let existing_matches = self
                    .instfiles
                    .iter()
                    .find(|instfile| instfile.path == normalized)
                    .is_some_and(|instfile| {
                        !instfile.is_content_modified(root.as_path()).unwrap_or(true)
                    });
                if existing_matches {
                    continue;
                }

                let bptnew_path = root.as_path().join(format!("{normalized}.bptnew"));
                let mut out = File::create(&bptnew_path)
                    .map_err(|e| Err::Write(bptnew_path.to_string(), e))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| Err::Write(bptnew_path.to_string(), e))?;
                bptnew.push(normalized);
                continue;
            }

            entry
                .unpack_in(root.as_path())
                .map_err(|e| Err::UnpackTarball(pkgid.to_string(), e))?;
        }
        Ok(())
    }

    pub fn into_file(self) -> File {
        self.tarball.into_inner().into_inner()
    }
}
