use crate::{
    cli::*, collection::*, color::Color, constant::*, error::*, file::*, io::*, location::*,
    marshalling::*, metadata::*, reconcile::*,
};
use std::cell::RefCell;

pub fn build(
    flags: CommonFlags,
    bbuilds: Vec<BbuildPathUrlRepo>,
    arch: Arch,
) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let netutil = &RefCell::new(NetUtil::new(bpt_conf, flags.netutil_stderr));
    let mut pkgcache = Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let available_bpts = &RefCell::new(AvailableBpts::new());
    let build_credentials = bpt_conf.build_credentials()?;
    let query_credentials = QueryCredentials::new(bpt_conf);

    // Determine the target [Arch] for each [Bbuild] while performing sanity checks.
    let mut bbuilds_archs = Vec::new();
    for bbuild in bbuilds {
        // The target [Arch] can be specified in two non-exclusive manners:
        //
        // - The `bbuild -a` argument, provided here as the `arch` parameter.
        // - The [PartId] `:arch` field.
        //
        // The `build -a` argument is populated with a default if left unspecified and effectively
        // cannot be opted out of.  However, the [PartId] `:arch` field can be left off.  The latter
        // takes precedence if specified and the former is only used if the latter is missing.
        let (bbuild, mut arch) = match bbuild {
            BbuildPathUrlRepo::Path(path) => {
                let bbuild = path.open(pubkeys, build_credentials.as_ref())?;
                (bbuild, arch)
            }
            BbuildPathUrlRepo::Url(url) => {
                let bbuild = url.download(
                    &netutil.borrow(),
                    &mut pkgcache,
                    pubkeys,
                    build_credentials.as_ref(),
                )?;
                (bbuild, arch)
            }
            BbuildPathUrlRepo::Repo(bpt_id) => {
                // If `:arch` was specified in the package id, use it.  Otherwise, use the `-a`
                // flag.
                let arch = bpt_id.arch.unwrap_or(arch);

                // fetch the bbuild needed to build the requested bpt from the repo
                let bbuild_id = bpt_id.with_arch(Arch::bbuild);
                let pkg_path_url = repository_pkgs
                    .best_pkg_match(&bbuild_id, &[Arch::bbuild])
                    .ok_or_else(|| Err::UnableToLocateAvailablePkg(bbuild_id.clone()))?
                    .repopath
                    .as_pkg_path_url()?;
                let pkg = pkg_path_url.open(
                    &netutil.borrow(),
                    &mut pkgcache,
                    pubkeys,
                    None,
                    Some(&query_credentials),
                )?;
                let bbuild = match pkg {
                    Pkg::Bpt(_) => {
                        return Err(Err::InvalidMagicNumber(
                            pkg_path_url.to_string(),
                            Bbuild::DESCRIPTION,
                        ));
                    }
                    Pkg::Bbuild(bbuild) => bbuild,
                };
                (bbuild, arch)
            }
        };

        // Sanity check arch
        match arch {
            Arch::bbuild => return Err(Err::BuildBbuild),
            Arch::native => return Err(Err::BuildNative),
            _ => {}
        }

        // A common workflow is to build all available packages into either either the native or
        // local architecture, unless the package is noarch, in which case noarch should be
        // targeted.  There isn't a good way to specify this to bpt.  As a work around, if the
        // package supports being built in an architecture-agnostic format, target that format
        // irrelevant of incoming architecture format request.
        if bbuild
            .pkginfo()
            .makearchs
            .as_slice()
            .contains(&Arch::noarch)
        {
            arch = Arch::noarch;
        }

        // Sanity check nothing exists at output path
        let out_filename = bbuild.pkgid().with_arch(arch).canonical_filename();
        let out_path = flags.out_dir.join(&out_filename);
        if out_path.exists() {
            return Err(Err::BuildOutputExists(
                out_filename,
                flags.out_dir,
                out_path,
            ));
        }

        bbuilds_archs.push((bbuild, arch));
    }

    let build_targets = sort_build_targets(
        bbuilds_archs
            .iter()
            .map(|(bbuild, arch)| BuildTarget {
                pkgid: bbuild.pkgid().with_arch(*arch),
                bbuild,
                arch: *arch,
            })
            .collect(),
    )?;

    if flags.dry_run {
        for target in &build_targets {
            println!(
                "{}Would{} build {}",
                Color::Warn,
                Color::Default,
                target.pkgid.color()
            );
        }
        println!();
        return Ok(format!(
            "Dry ran build of {} package(s)",
            build_targets.len()
        ));
    }

    // Delay collecting privkey, tmpdir, and src_cache until after confirming we actually need to
    // build, to avoid an unnecessary privkey password prompt and lock acquisition.
    let tmpdir = &RefCell::new(TmpDir::new(bpt_conf)?);
    let src_cache = &RefCell::new(Cache::from_root_path(
        &flags.root_dir,
        SRC_CACHE,
        "source cache",
    )?);
    let buildargs = BuildArgs {
        privkey: &PrivKey::from_common_flags(&flags)?,
        build_credentials: build_credentials.as_ref(),
        make_conf: &MakeConf::from_root_path(&flags.root_dir)?,
        make_common: &MakeCommon::from_root_path(&flags.root_dir)?,
        root_dir: &flags.root_dir.adjust_bedrock_prefix()?,
        out_dir: &flags.out_dir,
        tmpdir,
        netutil,
        src_cache,
        installed_pkgs,
        available_bpts,
    };

    // All binary packages in the repository are available.
    for pkgid in repository_pkgs.pkgids() {
        if pkgid.arch == Arch::bbuild {
            continue;
        }

        let partid = pkgid.to_pkgidpart();
        let pkg_path_url = repository_pkgs
            .best_pkg_match(&partid, &[pkgid.arch])
            .ok_or_else(|| Err::UnableToLocateAvailablePkg(partid.clone()))?
            .repopath
            .as_pkg_path_url()?;
        let pkg = pkg_path_url.open(&netutil.borrow(), &mut pkgcache, pubkeys, None, None)?;
        let bpt = match pkg {
            Pkg::Bpt(bpt) => bpt,
            Pkg::Bbuild(_) => {
                return Err(Err::InvalidMagicNumber(
                    pkg_path_url.to_string(),
                    Bpt::DESCRIPTION,
                ));
            }
        };
        available_bpts.borrow_mut().add(bpt);
    }

    // Up to this point we needed the package cache to check for cached remote packages.
    // We don't need it any more, and the following build step can take a while.  Free the package
    // cache to avoid unnecessarily blocking other bpt instances.
    drop(pkgcache);

    // Build the bbuilds into bpts.
    let mut bpts = Vec::new();
    for target in build_targets {
        let bpt = target.bbuild.build(&buildargs, target.arch)?;
        let filename = bpt.pkgid().canonical_filename();
        bpt.link(&flags.out_dir.join(&filename))?;
        available_bpts.borrow_mut().add(bpt);
        bpts.push(filename);
    }

    println!();

    if bpts.len() == 1 {
        Ok(format!("Built {}", bpts[0]))
    } else {
        Ok(format!("Built all {} packages", bpts.len()))
    }
}
