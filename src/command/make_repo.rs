use crate::constant::*;
use crate::{cli::*, collection::*, color::Color, error::*, file::*, io::*, reconcile::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::{cell::RefCell, time::SystemTime};

/// Filter the list of files by the specified file type, then load the given file type.
macro_rules! current {
    ($files:ident, $ext:literal, $ty:ty, $pubkeys:ident) => {
        $files
            .iter()
            .filter(|(path, _)| path.extension() == Some($ext))
            .map(|(path, mtime)| {
                <$ty>::from_file(File::open_ro(path)?, $pubkeys)
                    .map(|file| (*mtime, (*path).as_path(), file))
                    .loc(path)
            })
            .collect::<Result<Vec<_>, _>>()
    };
}

/// Collect all the file paths and their modified times currently in the given directory
fn current_files(dir: &Utf8Path) -> Result<Vec<(Utf8PathBuf, SystemTime)>, Err> {
    dir.readdir()?
        .map(|path| {
            path.metadata()
                .and_then(|meta| meta.modified())
                .map_err(|e| Err::Stat(path.to_owned(), e))
                .map(|mtime| (path, mtime))
        })
        .collect::<Result<Vec<_>, _>>()
}

pub fn make_repo(flags: CommonFlags) -> Result<String, Err> {
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let build_credentials = bpt_conf.build_credentials()?;

    let files = current_files(&flags.out_dir)?;
    let bbuilds = files
        .iter()
        .filter(|(path, _)| path.extension() == Some("bbuild"))
        .map(|(path, mtime)| {
            Bbuild::from_file(File::open_ro(path)?, pubkeys, build_credentials.as_ref())
                .map(|file| (*mtime, (*path).as_path(), file))
                .loc(path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if bbuilds.is_empty() {
        return Err(Err::MakeRepoNoBbuilds(flags.out_dir.to_owned()));
    }
    let bpts = current!(files, "bpt", Bpt, pubkeys)?;
    let pkgidxs = current!(files, "pkgidx", PkgIdx, pubkeys)?;
    let fileidxs = current!(files, "fileidx", FileIdx, pubkeys)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let mut available_bpts_new = AvailableBpts::new();
    for (_, path, _) in &bpts {
        let bpt = Bpt::from_file(File::open_ro(path)?, pubkeys).loc(path)?;
        available_bpts_new.add(bpt);
    }
    let available_bpts = &RefCell::new(available_bpts_new);

    let bpt_rec = BptReconciler::new(&bbuilds, &bpts, bpt_conf);
    let pkgidx_rec = PkgIdxReconciler::new(&bbuilds, &pkgidxs, bpt_conf)?;
    let fileidx_rec = FileIdxReconciler::new(&bbuilds, &fileidxs, bpt_conf)?;

    let bpt_plan = bpt_rec.plan();
    let pkgidx_plan = pkgidx_rec.plan();
    let fileidx_plan = fileidx_rec.plan();

    if bpt_plan.is_empty() && pkgidx_plan.is_empty() && fileidx_plan.is_empty() {
        return Ok("No changes needed".to_string());
    }

    if flags.dry_run {
        println!(
            "{}Would have:{}\n{bpt_plan}{pkgidx_plan}{fileidx_plan}",
            Color::Warn,
            Color::Default
        );
        return Ok(format!(
            "Dry ran make-repo of {} *.bbuild file(s)",
            bbuilds.len()
        ));
    }

    println!("Continuing will:\n{bpt_plan}{pkgidx_plan}{fileidx_plan}");
    if !flags.yes && !confirm()? {
        return Err(Err::ConfirmDenied);
    }

    // Typically, resources like privkey are collected at the very top of a per-command function.
    // However, here we want to delay collecting privkey until after confirming we actually need to
    // make changes to to avoid an unnecessary privkey password prompt.
    let privkey = &PrivKey::from_common_flags(&flags)?;

    let netutil = &RefCell::new(NetUtil::new(bpt_conf, flags.netutil_stderr));
    let tmpdir = &RefCell::new(TmpDir::new(bpt_conf)?);
    let src_cache = &RefCell::new(Cache::from_root_path(
        &flags.root_dir,
        SRC_CACHE,
        "source cache",
    )?);

    let buildargs = BuildArgs {
        privkey,
        build_credentials: build_credentials.as_ref(),
        make_conf: &MakeConf::from_root_path(&flags.root_dir)?,
        make_common: &MakeCommon::from_root_path(&flags.root_dir)?,
        out_dir: &flags.out_dir,
        root_dir: &flags.root_dir.adjust_bedrock_prefix()?,
        tmpdir,
        netutil,
        src_cache,
        installed_pkgs,
        available_bpts,
    };
    bpt_plan.apply(&buildargs)?;
    pkgidx_plan.apply(&PkgIdxRecArgs {
        dir: &flags.out_dir,
        privkey,
        pubkeys,
    })?;
    fileidx_plan.apply(&FileIdxRecArgs {
        dir: &flags.out_dir,
        privkey,
        pubkeys,
    })?;

    Ok(format!(
        "Updated repository files from {} *.bbuild file(s)",
        bbuilds.len()
    ))
}
