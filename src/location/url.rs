//! http(s) URL

use crate::color::*;
use crate::make_display_color;
use crate::{error::*, io::*};
use std::fs::File;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Url(String);

make_display_color!(Url, |s, f| {
    write!(f, "{}{}{}", Color::Url, s.0, Color::Default)
});

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Url {
    pub fn download(&self, netutil: &NetUtil, file: &mut File) -> Result<(), Err> {
        netutil.download(self, file)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Url {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(Self(s.to_owned()))
        } else {
            Err(Err::InvalidUrl(s.to_owned()))
        }
    }
}
