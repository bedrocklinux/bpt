//! http(s) URL to a [Bpt] or [Bbuild] file.

use crate::collection::{Cache, CacheResult};
use crate::error::*;
use crate::file::*;
use crate::io::*;
use crate::location::*;
use camino::Utf8Path;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PkgUrl(Url);

impl std::fmt::Display for PkgUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PkgUrl {
    pub fn download(
        &self,
        netutil: &NetUtil,
        pkgcache: &mut Cache,
        pubkeys: &PublicKeys,
        dir: Option<&Utf8Path>,
        query_credentials: Option<&QueryCredentials>,
    ) -> Result<Pkg, Err> {
        let file = match pkgcache.get(&self.0)? {
            CacheResult::Found(mut file) => match dir {
                Some(dir) => file.clone_anon_into(dir)?,
                None => file,
            },
            CacheResult::NewEntry(mut cache_file) => {
                netutil.download(&self.0, &mut cache_file)?;
                // Check pkg is valid before linking or copying
                let pkg = Pkg::from_file(cache_file, pubkeys, query_credentials, self.0.as_str())?
                    .ok_or_else(|| Err::InvalidPkgUrl(self.0.to_string()))?;
                // Valid, link and optionally copy
                pkg.link(&pkgcache.cache_path(&self.0))?;
                let mut file = pkg.into_file();
                match dir {
                    Some(dir) => file.clone_anon_into(dir)?,
                    None => file,
                }
            }
        };
        Pkg::from_file(file, pubkeys, query_credentials, self.0.as_str())?
            .ok_or_else(|| Err::InvalidPkgUrl(self.0.to_string()))
    }
}

impl std::ops::Deref for PkgUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for PkgUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::from_str(s)
            .map(PkgUrl)
            .map_err(|_| Err::InvalidPkgUrl(s.to_owned()))
    }
}
