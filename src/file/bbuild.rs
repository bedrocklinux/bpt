use crate::{
    collection::*, color::Color, constant::*, error::*, file::*, io::*, location::RootDir,
    marshalling::*, metadata::*, str::UnderscoreEncode,
};
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::CString,
    fs::{self, File},
    os::fd::{AsFd, BorrowedFd},
    os::unix::ffi::OsStrExt,
};

/// Package build definition
///
/// Filenames typically end in `.bbuild`.
pub struct Bbuild {
    file: BoundedFile,
    pkginfo: PkgInfo,
    pkgsrcs: Vec<PkgSrc>,
}

/// Values needed to build a package
pub struct BuildArgs<'a> {
    pub privkey: &'a PrivKey,
    pub build_credentials: Option<&'a ProcessCredentials>,
    pub make_conf: &'a MakeConf,
    pub make_common: &'a MakeCommon,
    // Root directory, incorporating both `-R`/`--root-dir` and, if targeting a local path on a
    // Bedrock system, the Bedrock stratum local path prefix.
    pub root_dir: &'a Utf8Path,
    pub out_dir: &'a Utf8Path,
    // By default, we delete temporary build artifacts to clean up after ourselves.  However,
    // impairs debugging should the build fail.  Thus, if a build fails, we set the retain_on_close
    // bit, and thus the type needs to be mutable.
    //
    // However, having the entire BuildArgs be mutable complicates the code base.  As such
    // failures should be rare for end-users, and we only ever build packages sequentially, we
    // plan around the assumption it doesn't happen and (ab)use RefCell for the rare case that it's
    // needed.
    pub tmpdir: &'a RefCell<TmpDir>,
    pub netutil: &'a RefCell<NetUtil<'a>>,
    pub src_cache: &'a RefCell<Cache>,
    pub installed_pkgs: &'a InstalledPkgs,
    pub available_bpts: &'a RefCell<AvailableBpts>,
}

impl MagicNumber for Bbuild {
    const DESCRIPTION: &'static str = "bbuild package build definition";
    const MAGIC: &'static [u8] = BBUILD_MAGIC;
}

/// Extract required metadata field from a HashMap
///
/// Errors if field is missing
macro_rules! extract {
    ($map:ident, $field:tt, $ty:ty) => {{
        let val = $map
            .remove($field)
            .ok_or_else(|| AnonLocErr::FieldMissing($field))?;
        let fstr = FieldStr::try_from(val).map_err(|e| e.field($field))?;
        <$ty>::from_field_str(fstr)
    }};
}

/// Extract optional metadata field from a HashMap
///
/// A missing field is treated as an empty string
macro_rules! extract_opt {
    ($map:ident, $field:tt, $ty:ty) => {{
        let val = $map.remove($field).unwrap_or("".to_string());
        let fstr = FieldStr::try_from(val).map_err(|e| e.field($field))?;
        <$ty>::from_field_str(fstr)
    }};
}

/// Build directory layout created for each package build
struct BuildDirs {
    builddir: TmpDir,
    pkgdir: TmpDir,
    workdir: TmpDir,
    log: TmpFile,
}

fn lchown_path(path: &Utf8Path, credentials: &ProcessCredentials) -> Result<(), Err> {
    let display = path.to_string();
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| {
        Err::Chown(
            display.clone(),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, e),
        )
    })?;
    // Safety: `c_path` is a valid NUL-terminated C string which `lchown` only borrows
    // for the duration of the syscall.
    if unsafe {
        nix::libc::lchown(
            c_path.as_ptr(),
            credentials.uid.as_raw(),
            credentials.gid.as_raw(),
        )
    } == 0
    {
        return Ok(());
    }

    Err(Err::Chown(display, std::io::Error::last_os_error()))
}

fn chown_tree(path: &Utf8Path, credentials: &ProcessCredentials) -> Result<(), Err> {
    lchown_path(path, credentials)?;

    let metadata = fs::symlink_metadata(path).map_err(|e| Err::Stat(path.to_owned(), e))?;
    if !metadata.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path).map_err(|e| Err::ReadDir(path.to_string(), e))? {
        let entry = entry.map_err(|e| Err::ReadDir(path.to_string(), e))?;
        let entry_path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            Err::Chown(
                path.display().to_string(),
                std::io::Error::new(std::io::ErrorKind::InvalidData, "non-utf8 path"),
            )
        })?;
        chown_tree(&entry_path, credentials)?;
    }

    Ok(())
}

impl Bbuild {
    pub fn from_file(
        file: File,
        pubkeys: &PublicKeys,
        query_credentials: Option<&ProcessCredentials>,
    ) -> Result<Self, AnonLocErr> {
        let file = file
            .verify_and_strip_sig(pubkeys)?
            .verify_and_strip_magic::<Self>()?;

        // bbuild files are shell scripts which define metadata in variables and a `build()` script
        // indicating how to build them.  Here we're just reading out the metadata by sourcing the
        // file and printing the variables we're interested in.  To do this, we need:
        //
        // - The file(s) we're sourcing
        // - Input variables the script expects
        // - Output variables we're extracting from the script
        //
        // Note we often source twice: once to get information about the script, then again if
        // we're actually going to build it.  The flow changes slightly between the two instances.

        // Scripts we're sourcing, as pre-opened file descriptors.
        //
        // When initially loading a bbuild, just source the bbuild file.  Later if/when we build
        // the package, we'll source supporting build scripts as well.
        let script_fds = &[file.as_fd()];

        // When building a bbuild, bpt populates variables about the target build such as what
        // architecture we want the resulting binary.  Bbuild scripts may be naively written with
        // the expectation those values are always populated even if we're just loading the file
        // and not building it, and thus we want to use placeholder values here.
        let input_vars: HashMap<&str, &str> = [
            ("pkgname", "to-be-determined"),
            ("pkgver", "to-be-determined"),
            ("arch", "host"),
            ("native_host_arch", "host"),
            ("pkgdir", "/tmp"),
            ("workdir", "/tmp"),
            ("rootdir", "/tmp"),
        ]
        .into_iter()
        .collect();

        // Values we want to get from the bbuild file.
        let output_vars = [
            "pkgname",
            "pkgver",
            "pkgdesc",
            "homepage",
            "license",
            "depends",
            "backup",
            "makearch",
            "makebins",
            "makedepends",
            "source",
            "sha256sums",
        ];

        // Source the script, populating provided variables and extracting resulting specified
        // variables.
        let mut vars =
            query_shell_scripts(script_fds, &input_vars, &output_vars, query_credentials)?;

        // The MakeArchs type supports being empty for bpt files.  However, it should not be
        // empty for bbuild files.  For a bbuild to be sane, it must be possible for it to build
        // into a package of some architecture.
        let makearchs = extract!(vars, "makearch", MakeArchs)?;
        if makearchs.as_slice().is_empty() {
            return Err(AnonLocErr::FieldEmpty("makearch"));
        }

        let source = vars.remove("source").unwrap_or("".to_string());
        let sha256sums = vars.remove("sha256sums").unwrap_or("".to_string());
        let pkgsrcs = PkgSrc::new_vec(&source, &sha256sums)?;

        Ok(Self {
            file,
            pkginfo: PkgInfo {
                pkgid: PkgId {
                    pkgname: extract!(vars, "pkgname", PkgName)?,
                    pkgver: extract!(vars, "pkgver", PkgVer)?,
                    arch: Arch::bbuild,
                },
                pkgdesc: extract!(vars, "pkgdesc", PkgDesc)?,
                homepage: extract!(vars, "homepage", Homepage)?,
                license: extract!(vars, "license", License)?,
                backup: extract_opt!(vars, "backup", Backup)?,
                depends: extract_opt!(vars, "depends", Depends)?,
                makedepends: extract_opt!(vars, "makedepends", MakeDepends)?,
                makearchs,
                makebins: extract_opt!(vars, "makebins", MakeBins)?,
                repopath: RepoPath::empty(),
            },
            pkgsrcs,
        })
    }

    pub fn build(&self, buildargs: &BuildArgs, arch: Arch) -> Result<Bpt, Err> {
        let (pkginfo, arch) = self.prepare_binary_pkginfo(arch)?;
        let pkgid = pkginfo.pkgid();
        self.pkginfo.makebins.confirm_available(pkgid)?;

        println!(
            "{}Building{} {}",
            Color::Create,
            Color::Default,
            pkgid.color()
        );

        let dirs = self.create_build_dirs(buildargs, pkgid)?;
        self.install_build_makedepends(buildargs, pkgid, arch, &dirs.workdir)?;
        self.fetch_sources(buildargs, &pkginfo, &dirs.workdir)?;
        let pkgdir_path = dirs.pkgdir.as_path().to_owned();
        if let Some(credentials) = buildargs.build_credentials.as_ref() {
            chown_tree(dirs.builddir.as_path(), credentials)?;
        }
        self.run_build_script(buildargs, &pkginfo, arch, dirs)?;

        Bpt::from_dir(&pkgdir_path, buildargs.out_dir, buildargs.privkey)
    }

    /// Prepare future binary pkginfo
    fn prepare_binary_pkginfo(&self, mut arch: Arch) -> Result<(PkgInfo, Arch), Err> {
        // Arch::bbuild is only intended for the bbuild file and doesn't make sense as a target
        // architecture for a binary.
        if arch == Arch::bbuild {
            return Err(Err::BuildBbuild);
        }

        // A common workflow is to build all available packages into either the native or host
        // architecture, unless the package is noarch, in which case noarch should be targeted.
        // There isn't a good way to explicitly specify this to bpt, and so we must detect it
        // implicitly.
        //
        // If the package supports being built in an architecture-agnostic format, target that
        // format irrelevant of incoming architecture format request.
        if self.pkginfo.makearchs.as_slice().contains(&Arch::noarch) {
            arch = Arch::noarch;
        }

        // Prepare binary package's pkginfo
        //
        // Clone the bbuild's, then adjust for any differences.
        let mut pkginfo = self.pkginfo.clone();
        pkginfo.pkgid.arch = arch;
        // Install/runtime dependencies for a package are usually aligned with that package's
        // architecture.  For example, htop:aarch64 depends on ncurses:aarch64 and htop:x86_64
        // depends on ncurses:x86_64.  A bbuild's depend values indicate this desired behavior for the
        // resulting binary's depends by leaving out the arch, in which case we populate it here.
        pkginfo.depends = pkginfo.depends.populate_depends_arch_if_missing(arch);
        // These are build-time relevant fields which only make sense for bbuild and not for built
        // binaries.  Thus, clear them.
        pkginfo.makearchs = MakeArchs::new();
        pkginfo.makebins = MakeBins::new();
        pkginfo.makedepends = MakeDepends::new();

        // Dereference native/host to actual target arch name, dropping how it relates to the
        // host.
        let arch = match arch {
            Arch::native => Arch::host(),
            _ => arch,
        };

        Ok((pkginfo, arch))
    }

    /// Create the build directory layout under the tmpdir.
    fn create_build_dirs(&self, buildargs: &BuildArgs, pkgid: &PkgId) -> Result<BuildDirs, Err> {
        let builddir_name = pkgid.to_string();
        let builddir_name = builddir_name.underscore_encode();
        let builddir = buildargs
            .tmpdir
            .borrow_mut()
            .subdir(Utf8Path::new(&builddir_name))?;
        let pkgdir = builddir.subdir(Utf8Path::new("pkg"))?;
        let workdir = builddir.subdir(Utf8Path::new("work"))?;
        let log = builddir.subfile(Utf8Path::new("stdout-stderr-log"))?;

        Ok(BuildDirs {
            builddir,
            pkgdir,
            workdir,
            log,
        })
    }

    /// Install any missing build dependencies into this build's workdir.
    ///
    /// Build scripts are expected to search both rootdir and workdir for dependencies.
    fn install_build_makedepends(
        &self,
        buildargs: &BuildArgs,
        pkgid: &PkgId,
        arch: Arch,
        workdir: &TmpDir,
    ) -> Result<(), Err> {
        let makedepends = self
            .pkginfo
            .makedepends
            .populate_depends_arch_if_missing(arch);
        if makedepends.as_slice().is_empty() {
            return Ok(());
        }

        let mut deps = makedepends.as_slice().to_vec();
        let mut resolved = HashSet::new();
        let mut workdir_pkgids = Vec::new();
        let mut in_workdir = HashSet::new();
        let candidate_archs = [arch, Arch::noarch];

        while let Some(depend) = deps.pop() {
            if !resolved.insert(depend.clone()) {
                continue;
            }

            if buildargs
                .installed_pkgs
                .best_provider(&depend, &candidate_archs)
                .is_some()
            {
                continue;
            }

            let provider_pkgid = buildargs
                .available_bpts
                .borrow()
                .best_provider_pkgid(&depend, &candidate_archs)
                .ok_or_else(|| {
                    Err::UnableToLocateDependency(Box::new(depend.clone()), pkgid.clone())
                })?;

            let provider_depends = buildargs
                .available_bpts
                .borrow()
                .get(&provider_pkgid)
                .ok_or_else(|| Err::UnableToLocateRepositoryPkg(provider_pkgid.to_pkgidpart()))?
                .pkginfo()
                .depends
                .as_slice()
                .to_vec();
            deps.extend(provider_depends);

            if in_workdir.insert(provider_pkgid.clone()) {
                workdir_pkgids.push(provider_pkgid);
            }
        }

        if workdir_pkgids.is_empty() {
            return Ok(());
        }

        let workdir_rootdir = RootDir::from_path(workdir.as_path());
        let workdir_instpkg_dir = workdir.subdir(Utf8Path::new(INSTPKG_DIR_PATH))?;
        // When running a user-facing install into the root_dir, this tracks .bptnew files they
        // should review/merge.  Here in the temporary workdir, it's safe to just ignore.
        let mut _bptnew = Vec::new();

        for workdir_pkgid in workdir_pkgids {
            buildargs
                .available_bpts
                .borrow_mut()
                .get_mut(&workdir_pkgid)
                .ok_or_else(|| Err::UnableToLocateRepositoryPkg(workdir_pkgid.to_pkgidpart()))?
                .install(
                    &workdir_rootdir,
                    workdir_instpkg_dir.as_path(),
                    &mut _bptnew,
                )?;
        }

        Ok(())
    }

    /// Download or retrieve from cache all source files, verifying checksums.
    fn fetch_sources(
        &self,
        buildargs: &BuildArgs,
        pkginfo: &PkgInfo,
        workdir: &TmpDir,
    ) -> Result<(), Err> {
        for pkgsrc in &self.pkgsrcs {
            let url = pkgsrc.url();
            let filepath = buildargs.src_cache.borrow().cache_path(url);
            let mut file = match buildargs.src_cache.borrow_mut().get(url)? {
                CacheResult::NewEntry(mut file) => {
                    buildargs.netutil.borrow_mut().download(url, &mut file)?;
                    if pkgsrc.check(&mut file).loc(url)? {
                        file.link(&filepath)?;
                    } else {
                        return Err(Err::SrcChecksumFailed(
                            pkginfo.pkgid.to_string(),
                            url.to_string(),
                        ));
                    }
                    file
                }
                CacheResult::Found(mut file) => {
                    if !pkgsrc.check(&mut file).loc(url)? {
                        println!(
                            "{}Removing cached source which failed checksum `{}`{}",
                            crate::color::Color::Warn,
                            &filepath,
                            crate::color::Color::Default,
                        );
                        std::fs::remove_file(&filepath)
                            .map_err(|e| Err::Remove(filepath.to_string(), e))?;
                        buildargs.netutil.borrow_mut().download(url, &mut file)?;
                        if pkgsrc.check(&mut file).loc(url)? {
                            file.link(&filepath)?;
                        } else {
                            return Err(Err::SrcChecksumFailed(
                                pkginfo.pkgid.to_string(),
                                url.to_string(),
                            ));
                        }
                        file
                    } else {
                        file
                    }
                }
            };

            let filename = pkgsrc.filename().unwrap_or("unnamed-source");
            file.copy_into_dir(workdir.as_path())?
                .link(&workdir.as_path().join(filename))?;
        }

        Ok(())
    }

    /// Source make.conf, make.common, and the bbuild script to run the build() function,
    /// then serialize the resulting pkginfo.
    fn run_build_script(
        &self,
        buildargs: &BuildArgs,
        pkginfo: &PkgInfo,
        arch: Arch,
        dirs: BuildDirs,
    ) -> Result<(), Err> {
        let pkgid = pkginfo.pkgid();

        // Bedrock provides `make.conf` and `make.common` support scripts.  Source these two files
        // before the bbuild script so the bbuild script can make use of what they provide.
        let script_fds = &[
            buildargs.make_conf.as_fd(),
            buildargs.make_common.as_fd(),
            self.file.as_fd(),
        ];

        // Build process details may differ depending on whether we're building portable vs
        // non-portable optimizations and whether we're targeting the host vs foreign
        // architecture.  Provide this information to the build scripts.
        //
        // See `assets/default-configs/make.conf` for example usage.
        let native_host_arch = match arch {
            Arch::native => Arch::native.as_str(),
            a if a == Arch::host() => "host",
            _ => arch.as_str(),
        };

        let pkgver = format!("{}", pkginfo.pkgid.pkgver);
        let pkgdir_str = dirs.pkgdir.as_str().to_owned();
        let workdir_str = dirs.workdir.as_str().to_owned();
        let input_vars: HashMap<&str, &str> = [
            ("pkgname", pkginfo.pkgid.pkgname.as_str()),
            ("pkgver", &pkgver),
            ("arch", arch.as_str()),
            ("native_host_arch", native_host_arch),
            ("pkgdir", &pkgdir_str),
            ("workdir", &workdir_str),
            ("rootdir", buildargs.root_dir.as_str()),
        ]
        .into_iter()
        .collect();

        let workdir_path = dirs.workdir.as_path().to_owned();
        let pkgdir_path = dirs.pkgdir.as_path().to_owned();
        let log = dirs.log.into_file();
        if run_shell_scripts(
            script_fds,
            &input_vars,
            "build",
            &workdir_path,
            log,
            buildargs.build_credentials,
        )
        .is_err()
        {
            // Normally build artifacts are deleted when bpt closes.  Retain them so they can be
            // investigated to understand why the build failed.
            buildargs.tmpdir.borrow_mut().retain_on_close();
            return Err(Err::BuildPkg(
                pkgid.to_string(),
                dirs.builddir.as_path().to_owned(),
            ));
        }

        // Create file describing package metadata
        let pkginfo_path = pkgdir_path.join(TARBALL_PKGINFO_PATH);
        let mut info_file = File::create(&pkginfo_path)
            .map_err(|e| Err::CreateFile(pkginfo_path.clone().into_string(), e))?;
        pkginfo.serialize(&mut info_file).loc(pkginfo_path)?;
        drop(info_file);

        Ok(())
    }

    pub fn pkginfo(&self) -> &PkgInfo {
        &self.pkginfo
    }

    pub fn pkgid(&self) -> &PkgId {
        self.pkginfo.pkgid()
    }

    pub fn into_file(self) -> File {
        self.file.into_inner()
    }

    pub fn select_make_arch(&self, archs: &[Arch]) -> Result<Arch, Err> {
        self.pkginfo.select_make_arch(archs)
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.file.inner().link(path)
    }
}

impl AsFd for Bbuild {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}
