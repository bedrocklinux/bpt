use crate::{
    color::*, constant::*, error::*, location::PkgPathUrl, make_display_color, make_field,
    marshalling::*, metadata::*, str::*,
};
use camino::Utf8Path;
use std::{borrow::Cow, str::FromStr};

/// Location where the package can be found within a repository.
///
/// Typically, this is populated with a path relative to the [crate::file::PkgIdx] and is thus
/// relocatable along with the [crate::file::PkgIdx].  However, by setting
/// [LOCATION_OVERRIDE_XATTR] on the package file, it may be overridden with an absolute http/https
/// URL or absolute filepath.
#[derive(Clone, Debug)]
pub struct RepoPath(FieldStr);

make_field!(RepoPath, PkgKey);

impl std::fmt::Display for RepoPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

make_display_color!(RepoPath, |s, f| {
    if s.0.starts_with("http://") || s.0.starts_with("https://") {
        write!(f, "{}{}{}", Color::Url, s, Color::Default)
    } else {
        write!(f, "{}{}{}", Color::File, s, Color::Default)
    }
});

impl FromFieldStr for RepoPath {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        Ok(Self(value))
    }
}

impl AsBytes for RepoPath {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_bytes())
    }
}

impl RepoPath {
    // RepoPath is only valid in pkgidxs, and should be empty in direct bpts and bbuilds.
    pub fn empty() -> Self {
        Self(FieldStr::empty())
    }

    pub fn from_path(path: &Utf8Path) -> Result<Self, Err> {
        let str = match xattr::get(path, LOCATION_OVERRIDE_XATTR) {
            // If overridden by xattr, use override value
            Ok(Some(xattr)) => xattr
                .into_string()
                .map_err(|e| Err::GetXattr(path.to_string(), e))?,
            // If no xattr, use filename (implicitly relative to pkgidx location)
            Ok(None) => path
                .file_name()
                .map(|path| path.to_owned())
                .ok_or_else(|| Err::PathLacksFileName(path.to_string()))?,
            // If the filesystem does not support xattrs and the user was interested in xattr
            // override, the user should have seen an error when trying to set them.  Thus, if we
            // get to this point and xattrs aren't supported, the user probably didn't want them in
            // the first place.  Continue as though no xattr override was specified.  That is, use
            // a filename (implicitly relative to pkgidx location).
            Err(e) if e.raw_os_error() == Some(nix::errno::Errno::EOPNOTSUPP as i32) => path
                .file_name()
                .map(|path| path.to_owned())
                .ok_or_else(|| Err::PathLacksFileName(path.to_string()))?,
            Err(e) => return Err(Err::GetXattr(path.to_string(), e)),
        };

        let fstr = FieldStr::try_from(str).field(Self::NAME).loc(path)?;

        Self::from_field_str(fstr).loc(path)
    }

    /// The location metadata may be relative to the pkgidx location.  If so, we need to make it an
    /// absolute path to consume it by prepending the pkgidx location.
    pub fn absolutize(&mut self, pkgidx_dir: &FieldStr) {
        if !self.0.starts_with('/')
            && !self.0.starts_with("http://")
            && !self.0.starts_with("https://")
        {
            let mut path = pkgidx_dir.clone();
            if !path.ends_with('/') {
                path.push('/').unwrap();
            }
            path.push_fieldstr(&self.0);
            self.0 = path
        }
    }

    pub fn as_pkg_path_url(&self) -> Result<PkgPathUrl, Err> {
        PkgPathUrl::from_str(&self.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_path(s: &str) -> RepoPath {
        let fstr = FieldStr::try_from(s).unwrap();
        RepoPath::from_field_str(fstr).unwrap()
    }

    fn field_str(s: &str) -> FieldStr {
        FieldStr::try_from(s).unwrap()
    }

    #[test]
    fn test_absolutize_relative_no_trailing_slash() {
        let mut rp = repo_path("foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs"));
        assert_eq!(rp.to_string(), "/repo/pkgs/foo.bpt");
    }

    #[test]
    fn test_absolutize_relative_with_trailing_slash() {
        let mut rp = repo_path("foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs/"));
        assert_eq!(rp.to_string(), "/repo/pkgs/foo.bpt");
    }

    #[test]
    fn test_absolutize_already_absolute_path() {
        let mut rp = repo_path("/other/path/foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs"));
        assert_eq!(rp.to_string(), "/other/path/foo.bpt");
    }

    #[test]
    fn test_absolutize_http_url() {
        let mut rp = repo_path("http://example.com/foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs"));
        assert_eq!(rp.to_string(), "http://example.com/foo.bpt");
    }

    #[test]
    fn test_absolutize_https_url() {
        let mut rp = repo_path("https://example.com/foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs"));
        assert_eq!(rp.to_string(), "https://example.com/foo.bpt");
    }

    #[test]
    fn test_absolutize_relative_subdir() {
        let mut rp = repo_path("subdir/foo.bpt");
        rp.absolutize(&field_str("/repo/pkgs"));
        assert_eq!(rp.to_string(), "/repo/pkgs/subdir/foo.bpt");
    }
}
