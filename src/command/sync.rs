use crate::{
    cli::*, collection::*, color::Color, constant::*, error::*, file::*, io::*, location::*,
    reconcile::*,
};
use std::fs::create_dir_all;

pub fn sync(flags: CommonFlags, idxs: Vec<IdxPathUrl>, force: bool) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let netutil = &NetUtil::new(bpt_conf, flags.netutil_stderr);

    let (idxs, remove_missing) = if idxs.is_empty() {
        (Repos::from_root_path(&flags.root_dir)?.into_vec(), true)
    } else {
        (idxs, false)
    };

    // Ensure the index directories exist
    let pkgidxdir = &flags.root_dir.as_path().join(PKGIDX_DIR_PATH);
    let fileidxdir = &flags.root_dir.as_path().join(FILEIDX_DIR_PATH);
    create_dir_all(pkgidxdir).map_err(|e| Err::CreateDir(pkgidxdir.to_string(), e))?;
    create_dir_all(fileidxdir).map_err(|e| Err::CreateDir(fileidxdir.to_string(), e))?;

    // Lock the index directories
    let _pkgidx_lock = pkgidxdir.lock_rw("available package index directory")?;
    let _fileidx_lock = fileidxdir.lock_rw("available file index directory")?;

    let rec = IdxReconciler::new(pkgidxdir, fileidxdir, &idxs, remove_missing)?;
    let plan = rec.plan();

    if plan.is_empty() {
        return Ok("No indexes configured; nothing to do".to_string());
    }

    if flags.dry_run {
        println!("{}Would have:{}\n{plan}", Color::Warn, Color::Default);
        return Ok(format!(
            "Dry ran synchronization of {} index(es)",
            idxs.len()
        ));
    }

    plan.apply(&IdxReconcilerArgs {
        pkgidxdir,
        fileidxdir,
        force,
        pubkeys,
        netutil,
    })?;

    // Evict stale cache entries on full sync (no CLI-specified indexes).
    if remove_missing {
        Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?
            .evict(bpt_conf.cache.cache_pkg_max_days)?;
        Cache::from_root_path(&flags.root_dir, SRC_CACHE, "source cache")?
            .evict(bpt_conf.cache.cache_src_max_days)?;
    }

    if idxs.len() == 1 {
        Ok(format!("Synchronized {}", idxs[0].as_str()))
    } else {
        Ok(format!("Synchronized {} indexes", idxs.len()))
    }
}
