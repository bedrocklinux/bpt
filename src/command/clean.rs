use crate::{cli::*, collection::*, color::Color, constant::*, error::*};

pub fn clean(flags: CommonFlags, mut packages: bool, mut source: bool) -> Result<String, Err> {
    let mut pkgcache = Cache::from_root_path(&flags.root_dir, PKG_CACHE, "package cache")?;
    let mut srccache = Cache::from_root_path(&flags.root_dir, SRC_CACHE, "source cache")?;
    let mut count = 0;

    if !packages && !source {
        packages = true;
        source = true;
    }

    if flags.dry_run {
        if packages {
            count += pkgcache.count_evictable(Some(0))?;
        }
        if source {
            count += srccache.count_evictable(Some(0))?;
        }
        Ok(format!(
            "{}Dry run would have removed{} {count} cached items",
            Color::Warn,
            Color::Default
        ))
    } else {
        if packages {
            count += pkgcache.evict(Some(0))?;
        }
        if source {
            count += srccache.evict(Some(0))?;
        }
        Ok(format!(
            "{}Removed{} {count} cached items",
            Color::Remove,
            Color::Default
        ))
    }
}
