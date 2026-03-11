use crate::{
    cli::*, collection::*, color::Color, constant::*, error::*, file::*, io::*, metadata::*,
};

pub fn fetch(flags: CommonFlags, pkgs: Vec<PartId>) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let netutil = &NetUtil::new(bpt_conf, flags.netutil_stderr);
    let query_credentials = QueryCredentials::new(bpt_conf);
    let mut pkgcache = Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;

    let mut resolved = Vec::new();
    for partid in &pkgs {
        let pkginfo = repository_pkgs
            .best_pkg_match(partid, &bpt_conf.general.default_archs)
            .ok_or_else(|| Err::UnableToLocateAvailablePkg(partid.clone()))?;
        resolved.push(pkginfo);
    }

    if flags.dry_run {
        for pkginfo in &resolved {
            println!(
                "{}Would{} fetch {}",
                Color::Warn,
                Color::Default,
                pkginfo.repopath.color()
            );
        }
        return Ok(format!("Dry ran fetch of {} package(s)", pkgs.len()));
    }

    let mut first = None;
    for pkginfo in resolved {
        let pkg = pkginfo.repopath.as_pkg_path_url()?.open(
            netutil,
            &mut pkgcache,
            pubkeys,
            Some(&flags.out_dir),
            Some(&query_credentials),
        )?;

        let filename = pkg.pkgid().canonical_filename();
        let path = flags.out_dir.join(&filename);
        pkg.link(&path)?;

        first = first.or(Some(filename));
    }

    if pkgs.len() == 1
        && let Some(first) = first
    {
        Ok(format!("Fetched {}", first))
    } else {
        Ok(format!("Fetched all {} packages", pkgs.len()))
    }
}
