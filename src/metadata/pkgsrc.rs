use crate::error::*;
use crate::location::Url;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::{io::Seek, str::FromStr};

/// Bbuild package source code information
// Enum in case we want to allow e.g. git later.
pub enum PkgSrc {
    HttpUrl { url: Url, checksum: Option<String> },
}

impl PkgSrc {
    pub fn new_vec(sources: &str, checksums: &str) -> Result<Vec<Self>, AnonLocErr> {
        let sources: Vec<&str> = sources.split_ascii_whitespace().collect();
        let checksums: Vec<&str> = checksums.split_ascii_whitespace().collect();

        if sources.len() != checksums.len() {
            return Err(AnonLocErr::SrcChecksumCountMismatch(
                sources.len(),
                checksums.len(),
            ));
        }

        let mut pkgsrcs = Vec::new();
        for (source, checksum) in sources.iter().zip(checksums.iter()) {
            let pkgsrc = PkgSrc::new(source, checksum)?;
            pkgsrcs.push(pkgsrc);
        }

        Ok(pkgsrcs)
    }

    pub fn new(source: &str, checksum: &str) -> Result<Self, AnonLocErr> {
        let url = crate::location::Url::from_str(source)
            .map_err(|_| AnonLocErr::FieldInvalid("source", source.to_owned()))?;

        let checksum = if checksum == "SKIP" {
            None
        } else {
            if checksum.len() != 64 || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(AnonLocErr::FieldInvalid("sha256sum", checksum.to_owned()));
            }
            Some(checksum.to_ascii_lowercase())
        };

        Ok(Self::HttpUrl { url, checksum })
    }

    pub fn url(&self) -> &crate::location::Url {
        match self {
            PkgSrc::HttpUrl { url, .. } => url,
        }
    }

    pub fn check(&self, file: &mut File) -> Result<bool, AnonLocErr> {
        let expect = match self {
            PkgSrc::HttpUrl {
                checksum: Some(expect),
                ..
            } => expect,
            PkgSrc::HttpUrl { checksum: None, .. } => return Ok(true),
        };

        file.rewind().map_err(AnonLocErr::Seek)?;
        let mut hasher = Sha256::new();
        std::io::copy(file, &mut hasher).map_err(AnonLocErr::Read)?;
        let actual = hasher.finalize();

        Ok(hex_eq(&actual, expect))
    }

    pub fn filename(&self) -> Option<&str> {
        let base = self.url().as_str().split(['?', '#']).next()?;
        let name = base.rsplit('/').next()?;
        if name.is_empty() { None } else { Some(name) }
    }
}

/// Compare a byte slice against a lowercase hex string without allocating.
fn hex_eq(bytes: &[u8], hex: &str) -> bool {
    if hex.len() != bytes.len() * 2 {
        return false;
    }
    let hex = hex.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        let hi = hex[i * 2];
        let lo = hex[i * 2 + 1];
        if hi != b"0123456789abcdef"[(b >> 4) as usize] {
            return false;
        }
        if lo != b"0123456789abcdef"[(b & 0xf) as usize] {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::*;
    use std::io::{Seek, SeekFrom};

    const TMPNAME: &std::ffi::CStr = c"test-pkgsrc";

    // SHA-256 of b"hello world"
    const HELLO_WORLD_SHA256: &str =
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
    // SHA-256 of empty content
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn make_pkgsrc(checksum: Option<&str>) -> PkgSrc {
        PkgSrc::HttpUrl {
            url: crate::location::Url::from_str("https://example.com/file.tar.gz").unwrap(),
            checksum: checksum.map(|s| s.to_owned()),
        }
    }

    #[test]
    fn check_matching_checksum() {
        let pkgsrc = make_pkgsrc(Some(HELLO_WORLD_SHA256));
        let mut file = File::create_memfd(TMPNAME, b"hello world").unwrap();
        assert!(pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn check_mismatched_checksum() {
        let pkgsrc = make_pkgsrc(Some(
            "0000000000000000000000000000000000000000000000000000000000000000",
        ));
        let mut file = File::create_memfd(TMPNAME, b"hello world").unwrap();
        assert!(!pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn check_none_skips_verification() {
        let pkgsrc = make_pkgsrc(None);
        let mut file = File::create_memfd(TMPNAME, b"hello world").unwrap();
        assert!(pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn check_rewinds_before_hashing() {
        let pkgsrc = make_pkgsrc(Some(HELLO_WORLD_SHA256));
        let mut file = File::create_memfd(TMPNAME, b"hello world").unwrap();
        // Move cursor to the middle of the file
        file.seek(SeekFrom::Start(5)).unwrap();
        // check() should rewind and still produce the correct hash
        assert!(pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn check_empty_file() {
        let pkgsrc = make_pkgsrc(Some(EMPTY_SHA256));
        let mut file = File::create_memfd(TMPNAME, b"").unwrap();
        assert!(pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn check_empty_file_wrong_checksum() {
        let pkgsrc = make_pkgsrc(Some(HELLO_WORLD_SHA256));
        let mut file = File::create_memfd(TMPNAME, b"").unwrap();
        assert!(!pkgsrc.check(&mut file).unwrap());
    }

    #[test]
    fn new_vec_matching_counts() {
        let pkgsrcs = PkgSrc::new_vec("https://a.com/x https://b.com/y", "SKIP SKIP").unwrap();
        assert_eq!(pkgsrcs.len(), 2);
    }

    #[test]
    fn new_vec_more_sources_than_checksums() {
        let result = PkgSrc::new_vec("https://a.com/x https://b.com/y", "SKIP");
        assert!(matches!(
            result,
            Err(AnonLocErr::SrcChecksumCountMismatch(2, 1))
        ));
    }

    #[test]
    fn new_vec_more_checksums_than_sources() {
        let result = PkgSrc::new_vec("https://a.com/x", "SKIP SKIP");
        assert!(matches!(
            result,
            Err(AnonLocErr::SrcChecksumCountMismatch(1, 2))
        ));
    }

    #[test]
    fn filename_normal() {
        let pkgsrc = make_pkgsrc(None);
        assert_eq!(pkgsrc.filename(), Some("file.tar.gz"));
    }

    #[test]
    fn filename_trailing_slash() {
        let pkgsrc = PkgSrc::HttpUrl {
            url: crate::location::Url::from_str("https://example.com/path/").unwrap(),
            checksum: None,
        };
        assert_eq!(pkgsrc.filename(), None);
    }

    #[test]
    fn filename_with_query_string() {
        let pkgsrc = PkgSrc::HttpUrl {
            url: crate::location::Url::from_str("https://example.com/file.tar.gz?v=1").unwrap(),
            checksum: None,
        };
        assert_eq!(pkgsrc.filename(), Some("file.tar.gz"));
    }

    #[test]
    fn filename_with_fragment() {
        let pkgsrc = PkgSrc::HttpUrl {
            url: crate::location::Url::from_str("https://example.com/file.tar.gz#sha256").unwrap(),
            checksum: None,
        };
        assert_eq!(pkgsrc.filename(), Some("file.tar.gz"));
    }

    #[test]
    fn new_vec_both_empty() {
        let pkgsrcs = PkgSrc::new_vec("", "").unwrap();
        assert!(pkgsrcs.is_empty());
    }

    #[test]
    fn hex_eq_match() {
        assert!(hex_eq(&[0x00, 0xff, 0xab, 0x12], "00ffab12"));
    }

    #[test]
    fn hex_eq_mismatch() {
        assert!(!hex_eq(&[0x00, 0xff], "00fe"));
    }

    #[test]
    fn hex_eq_wrong_length() {
        assert!(!hex_eq(&[0x00], "000"));
        assert!(!hex_eq(&[0x00, 0x01], "00"));
    }

    #[test]
    fn hex_eq_empty() {
        assert!(hex_eq(&[], ""));
    }

    #[test]
    fn hex_eq_uppercase_rejected() {
        assert!(!hex_eq(&[0xab], "AB"));
    }

    #[test]
    fn hex_eq_all_byte_values() {
        let bytes: Vec<u8> = (0..=255).collect();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert!(hex_eq(&bytes, &hex));
    }

    #[test]
    fn new_valid_checksum() {
        let pkgsrc = PkgSrc::new("https://example.com/f", HELLO_WORLD_SHA256).unwrap();
        match pkgsrc {
            PkgSrc::HttpUrl { checksum, .. } => {
                assert_eq!(checksum.as_deref(), Some(HELLO_WORLD_SHA256));
            }
        }
    }

    #[test]
    fn new_skip_checksum() {
        let pkgsrc = PkgSrc::new("https://example.com/f", "SKIP").unwrap();
        match pkgsrc {
            PkgSrc::HttpUrl { checksum, .. } => assert_eq!(checksum, None),
        }
    }

    #[test]
    fn new_uppercase_checksum_normalized() {
        let upper = HELLO_WORLD_SHA256.to_ascii_uppercase();
        let pkgsrc = PkgSrc::new("https://example.com/f", &upper).unwrap();
        match pkgsrc {
            PkgSrc::HttpUrl { checksum, .. } => {
                assert_eq!(checksum.as_deref(), Some(HELLO_WORLD_SHA256));
            }
        }
    }

    #[test]
    fn new_checksum_too_short() {
        let result = PkgSrc::new("https://example.com/f", "abcd");
        assert!(matches!(
            result,
            Err(AnonLocErr::FieldInvalid("sha256sum", _))
        ));
    }

    #[test]
    fn new_checksum_too_long() {
        let long = "a".repeat(65);
        let result = PkgSrc::new("https://example.com/f", &long);
        assert!(matches!(
            result,
            Err(AnonLocErr::FieldInvalid("sha256sum", _))
        ));
    }

    #[test]
    fn new_checksum_non_hex() {
        let bad = "g".repeat(64);
        let result = PkgSrc::new("https://example.com/f", &bad);
        assert!(matches!(
            result,
            Err(AnonLocErr::FieldInvalid("sha256sum", _))
        ));
    }
}
