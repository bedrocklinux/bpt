use crate::{
    cli::*, collection::*, constant::*, error::*, file::*, io::*, location::*, metadata::*,
};
use camino::Utf8PathBuf;

pub fn files(
    flags: CommonFlags,
    pkgs: Vec<BptPathUrlRepo>,
    mut installed: bool,
    mut repository: bool,
) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let netutil = &NetUtil::new(bpt_conf, flags.netutil_stderr);
    let mut pkgcache = Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let archs = &bpt_conf.general.default_archs;
    let pkg_files = PkgFiles::from_root_path(&flags.root_dir, pubkeys)?;

    if !installed && !repository {
        installed = true;
        repository = true;
    }

    let mut pkg_paths: Vec<(PkgId, Vec<Utf8PathBuf>)> = Vec::new();
    for pkg in &pkgs {
        match pkg {
            BptPathUrlRepo::Path(path) => {
                let bpt = path.open(pubkeys, None)?;
                pkg_paths.push((bpt.pkgid().to_owned(), bpt.filepaths().to_owned()))
            }
            BptPathUrlRepo::Url(url) => {
                let bpt = url.download(netutil, &mut pkgcache, pubkeys, None)?;
                pkg_paths.push((bpt.pkgid().to_owned(), bpt.filepaths().to_owned()))
            }
            BptPathUrlRepo::Repo(partid) => pkg_paths.push(resolve_partid_paths(
                partid,
                installed,
                repository,
                installed_pkgs,
                repository_pkgs,
                &pkg_files,
                archs,
            )?),
        }
    }

    pkg_paths.sort_by(|(pkgid1, _), (pkgid2, _)| pkgid1.cmp(pkgid2));
    pkg_paths.dedup_by(|(pkgid1, _), (pkgid2, _)| pkgid1 == pkgid2);

    let width = pkg_paths
        .iter()
        .map(|(pkgid, _)| pkgid.color().to_string().len() + 1)
        .max()
        .unwrap_or(0);

    for (pkgid, paths) in pkg_paths {
        // Pre-formatting pkgid is needed for format!() padding to work.
        let pkgid = pkgid.color().to_string();

        for path in paths {
            println!("{pkgid:width$}{path}");
        }
    }

    // This is likely to be parsed by other programs. Do not complicate output by printing a
    // success message.
    Ok(String::new())
}

fn resolve_partid_paths(
    partid: &crate::metadata::PartId,
    installed: bool,
    repository: bool,
    installed_pkgs: &InstalledPkgs,
    repository_pkgs: &RepositoryPkgs,
    pkg_files: &PkgFiles,
    archs: &[crate::metadata::Arch],
) -> Result<(PkgId, Vec<Utf8PathBuf>), Err> {
    if installed && let Some(instpkg) = installed_pkgs.best_match(partid, archs) {
        return Ok((
            instpkg.pkgid().clone(),
            instpkg.paths().map(|p| p.to_path_buf()).collect(),
        ));
    }

    if repository
        && let Some(pkginfo) = repository_pkgs.best_pkg_match(partid, archs)
        && let Some((pkgid, paths)) = pkg_files
            .pkgid_paths()
            .find(|(pkgid, _)| *pkgid == pkginfo.pkgid())
    {
        return Ok((
            pkgid.to_owned(),
            paths.iter().map(|p| p.to_path_buf()).collect(),
        ));
    }

    if installed && !repository {
        Err(Err::UnableToLocateInstalledPkg(partid.clone()))
    } else {
        Err(Err::UnableToLocateAvailablePkg(partid.clone()))
    }
}
