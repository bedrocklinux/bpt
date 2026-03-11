//! http(s) URL to a [Bbuild] file.

use crate::{collection::*, error::*, file::*, io::*, location::*};
use std::str::FromStr;

#[derive(Clone)]
pub struct BbuildUrl(Url);

impl BbuildUrl {
    pub fn download(
        &self,
        netutil: &NetUtil,
        pkgcache: &mut Cache,
        pubkeys: &PublicKeys,
        query_credentials: Option<&ProcessCredentials>,
    ) -> Result<Bbuild, Err> {
        let mut link_in_cache = false;

        let file = match pkgcache.get(&self.0)? {
            CacheResult::Found(file) => file,
            CacheResult::NewEntry(mut file) => {
                link_in_cache = true;
                netutil.download(&self.0, &mut file)?;
                file
            }
        };

        let bbuild = Bbuild::from_file(file, pubkeys, query_credentials).loc(self.0.as_str())?;

        // We know:
        // - The package cache is available
        // - The package cache lacks an entry for this URL
        // - The URL is a valid, signature-verified Bbuild file
        // And so it's safe to link into the cache
        if link_in_cache {
            bbuild.link(&pkgcache.cache_path(&self.0))?;
        }

        Ok(bbuild)
    }
}

impl FromStr for BbuildUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::from_str(s)
            .map(BbuildUrl)
            .map_err(|_| Err::InvalidBbuildUrl(s.to_owned()))
    }
}
