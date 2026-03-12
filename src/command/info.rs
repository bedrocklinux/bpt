use crate::{cli::*, collection::*, constant::*, error::*, file::*, io::*, location::*};

pub fn info(
    flags: CommonFlags,
    pkgs: Vec<PkgPathUrlRepo>,
    mut installed: bool,
    mut repository: bool,
) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let netutil = &NetUtil::new(bpt_conf, flags.netutil_stderr);
    let query_credentials = QueryCredentials::new(bpt_conf);
    let mut pkgcache = Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
    let archs = &bpt_conf.general.default_archs;

    if !installed && !repository {
        installed = true;
        repository = true;
    }

    let mut pkginfos = Vec::new();
    for pkg in &pkgs {
        let pkginfo = match pkg {
            PkgPathUrlRepo::Path(path) => path
                .open(pubkeys, None, Some(&query_credentials))?
                .pkginfo()
                .clone(),
            PkgPathUrlRepo::Url(url) => url
                .download(
                    netutil,
                    &mut pkgcache,
                    pubkeys,
                    None,
                    Some(&query_credentials),
                )?
                .pkginfo()
                .clone(),
            PkgPathUrlRepo::Repo(partid) => resolve_partid_pkginfo(
                partid,
                installed,
                repository,
                installed_pkgs,
                repository_pkgs,
                archs,
            )?,
        };
        pkginfos.push(pkginfo);
    }

    for pkginfo in pkginfos {
        println!("{}", pkginfo.color());
    }

    // This is likely to be parsed by other programs.  Do not complicate output by printing a
    // success message.
    Ok(String::new())
}

fn resolve_partid_pkginfo(
    partid: &crate::metadata::PartId,
    installed: bool,
    repository: bool,
    installed_pkgs: &InstalledPkgs,
    repository_pkgs: &RepositoryPkgs,
    archs: &[crate::metadata::Arch],
) -> Result<crate::metadata::PkgInfo, Err> {
    if installed && let Some(instpkg) = installed_pkgs.best_match(partid, archs) {
        return Ok(instpkg.pkginfo().clone());
    }

    if repository && let Some(pkginfo) = repository_pkgs.best_pkg_match(partid, archs) {
        return Ok(pkginfo.clone());
    }

    if installed && !repository {
        Err(Err::UnableToLocateInstalledPkg(partid.clone()))
    } else {
        Err(Err::UnableToLocateAvailablePkg(partid.clone()))
    }
}
