//! http(s) URL to a [Bpt] file.

use crate::{collection::*, error::*, file::*, io::*, location::*, marshalling::VerifyMagic};
use std::io::{Seek, SeekFrom};
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BptUrl(Url);

impl std::fmt::Display for BptUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BptUrl {
    pub fn download(
        &self,
        netutil: &NetUtil,
        pkgcache: &mut Cache,
        pubkeys: &PublicKeys,
        dir: Option<&camino::Utf8Path>,
    ) -> Result<Bpt, Err> {
        let file = match pkgcache.get(&self.0)? {
            CacheResult::Found(mut file) => {
                if file.verify_magic::<Bpt>().is_err() {
                    return Err(Err::InvalidBptUrl(self.0.to_string()));
                }
                file.seek(SeekFrom::Start(0))
                    .map_err(|e| Err::Seek(self.0.to_string(), e))?;
                match dir {
                    Some(dir) => file.clone_anon_into(dir)?,
                    None => file,
                }
            }
            CacheResult::NewEntry(mut cache_file) => {
                netutil.download(&self.0, &mut cache_file)?;
                if cache_file.verify_magic::<Bpt>().is_err() {
                    return Err(Err::InvalidBptUrl(self.0.to_string()));
                }
                cache_file
                    .seek(SeekFrom::Start(0))
                    .map_err(|e| Err::Seek(self.0.to_string(), e))?;
                let bpt = Bpt::from_file(cache_file, pubkeys).loc(self)?;
                bpt.link(&pkgcache.cache_path(&self.0))?;

                let mut file = bpt.into_file();
                match dir {
                    Some(dir) => file.clone_anon_into(dir)?,
                    None => file,
                }
            }
        };

        Bpt::from_file(file, pubkeys).loc(self)
    }
}

impl std::ops::Deref for BptUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for BptUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::from_str(s)
            .map(BptUrl)
            .map_err(|_| Err::InvalidBptUrl(s.to_owned()))
    }
}
