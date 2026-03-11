//! http(s) URL to a [PkgIdx] or [FileIdx] file.

use crate::{error::*, file::*, io::*, location::*, make_display_color};
use camino::Utf8Path;
use std::fs::File;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdxUrl(Url);

make_display_color!(IdxUrl, |s, f| {
    // The embedded [Url] type has colors defined
    write!(f, "{}", s.0.color())
});

impl std::fmt::Display for IdxUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl IdxUrl {
    pub fn download(
        &self,
        netutil: &NetUtil,
        pubkeys: &PublicKeys,
        dir: Option<&Utf8Path>,
    ) -> Result<Idx, Err> {
        let mut file = File::create_anon(dir.unwrap_or_else(|| Utf8Path::new("/tmp")))?;
        self.0.download(netutil, &mut file)?;
        Idx::from_file(file, pubkeys)
            .loc(self)?
            .ok_or_else(|| Err::InvalidIdxUrl(self.0.to_string()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for IdxUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for IdxUrl {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::from_str(s)
            .map(IdxUrl)
            .map_err(|_| Err::InvalidIdxUrl(s.to_owned()))
    }
}
