pub trait StripFilename {
    fn strip_filename(&self) -> &str;
}

impl StripFilename for str {
    fn strip_filename(&self) -> &str {
        self.rfind('/').map(|i| &self[..i]).unwrap_or("")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_stripfilename() {
        assert_eq!(
            "http://example.com/foo/bar.pkgidx".strip_filename(),
            "http://example.com/foo"
        );
        assert_eq!(
            "https://example.com/foo/bar.pkgidx".strip_filename(),
            "https://example.com/foo"
        );
        assert_eq!("/foo/bar.pkgidx".strip_filename(), "/foo");
        assert_eq!("bar.pkgidx".strip_filename(), "");
    }
}
