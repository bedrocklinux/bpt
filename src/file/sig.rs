use crate::{constant::*, error::*, str::Base64Decode};
use std::io::{Read, Seek, SeekFrom};

/// Signature format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigFormat {
    /// v1: minisign Ed25519 (SignatureBones, 74 bytes raw / 99 chars base64)
    V1,
}

/// Location of a signature found within a file's tail region.
///
/// Shared between pubkeys and privkey logic
pub struct SigLocation {
    /// Total file length
    pub file_len: u64,
    /// File length excluding the signature (i.e. content only)
    pub content_len: u64,
    /// Raw base64 bytes of the signature (not yet decoded)
    pub sig_base64: Vec<u8>,
    /// Which signature format version was found
    pub format: SigFormat,
}

/// Result of searching for a signature in a file's tail.
pub enum FindSigResult {
    /// No signature prefix/suffix structure found at all.
    NotFound,
    /// Signature prefix and suffix found, but the base64 content between them is corrupt.
    Corrupt,
    /// Valid signature structure found.
    Found(SigLocation),
}

pub trait FindSignatureBlock: Read + Seek {
    fn find_signature_block(&mut self) -> Result<Option<SigLocation>, AnonLocErr> {
        let file_len = self.seek(SeekFrom::End(0)).map_err(AnonLocErr::Seek)?;
        let read_len = std::cmp::min(file_len, SIG_LEN_MAX);

        // Minimum: stem + version_tag + ":" + 1 base64 byte + suffix
        if (read_len as usize) < SIG_PREFIX_STEM.len() + 2 + 1 + SIG_SUFFIX.len() {
            return Ok(None);
        }

        self.seek(SeekFrom::End(-(read_len as i64)))
            .map_err(AnonLocErr::Seek)?;
        let mut buf = Vec::<u8>::with_capacity(read_len as usize);
        self.read_to_end(&mut buf).map_err(AnonLocErr::Read)?;

        // Find the common stem
        let stem_start = match buf
            .windows(SIG_PREFIX_STEM.len())
            .rposition(|window| window == SIG_PREFIX_STEM.as_bytes())
        {
            Some(pos) => pos,
            None => return Ok(None),
        };

        if !buf[stem_start..].ends_with(SIG_SUFFIX.as_bytes()) {
            return Ok(None);
        }

        // Extract version tag: bytes between stem and ':'
        let after_stem = stem_start + SIG_PREFIX_STEM.len();
        let colon_pos = match buf[after_stem..buf.len() - SIG_SUFFIX.len()]
            .iter()
            .position(|&b| b == b':')
        {
            Some(pos) => after_stem + pos,
            None => return Ok(None),
        };
        let version_tag = &buf[after_stem..colon_pos];

        let format = match version_tag {
            b"v1" => SigFormat::V1,
            _ => return Ok(None), // Unknown version, treat as no signature found
        };

        let full_prefix_len = colon_pos - stem_start + 1; // includes ':'
        let sig_base64 = &buf[colon_pos + 1..buf.len() - SIG_SUFFIX.len()];
        let sig_size = (full_prefix_len + sig_base64.len() + SIG_SUFFIX.len()) as u64;

        Ok(Some(SigLocation {
            file_len,
            content_len: file_len - sig_size,
            sig_base64: sig_base64.to_vec(),
            format,
        }))
    }

    fn find_signature(&mut self) -> Result<FindSigResult, AnonLocErr> {
        let Some(sig_loc) = self.find_signature_block()? else {
            return Ok(FindSigResult::NotFound);
        };

        if sig_loc.sig_base64.base64_decode().is_err() {
            return Ok(FindSigResult::Corrupt);
        }

        Ok(FindSigResult::Found(sig_loc))
    }
}

impl<T: Read + Seek> FindSignatureBlock for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::FileAux;
    use std::fs::File;

    // A valid base64-encoded minisign signature (from existing test fixtures)
    const VALID_SIG_B64: &str = "RUSWg+V4uzz1zRLiMvYdSiKjPd86/ZZC8TYnsmwrPsYTr2NUmnG5fN+sHoLg90YU2tNXtYscxROVXgYh+O/L/R4/Z3wZKhjZ8QA";

    fn make_file(contents: &[u8]) -> File {
        File::create_memfd(c"sig-test", contents).unwrap()
    }

    fn make_signed_file(body: &[u8], sig_b64: &str) -> File {
        let mut contents = body.to_vec();
        contents.extend_from_slice(SIG_V1_PREFIX.as_bytes());
        contents.extend_from_slice(sig_b64.as_bytes());
        contents.extend_from_slice(SIG_SUFFIX.as_bytes());
        make_file(&contents)
    }

    // ------------------------------------------------------------------
    // find_signature_block tests
    // ------------------------------------------------------------------

    #[test]
    fn test_find_signature_block_found() {
        let body = b"file content";
        let mut file = make_signed_file(body, VALID_SIG_B64);

        let result = file.find_signature_block().unwrap();
        let loc = result.expect("expected Some(SigLocation)");

        assert_eq!(loc.content_len, body.len() as u64);
        assert_eq!(loc.sig_base64, VALID_SIG_B64.as_bytes());
        let expected_len =
            body.len() + SIG_V1_PREFIX.len() + VALID_SIG_B64.len() + SIG_SUFFIX.len();
        assert_eq!(loc.file_len, expected_len as u64);
    }

    #[test]
    fn test_find_signature_block_no_prefix() {
        let mut file = make_file(b"just some plain content");
        let result = file.find_signature_block().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_signature_block_no_suffix() {
        let mut contents = b"body".to_vec();
        contents.extend_from_slice(SIG_V1_PREFIX.as_bytes());
        contents.extend_from_slice(b"c29tZWJhc2U2NA");
        // No SIG_SUFFIX appended
        let mut file = make_file(&contents);

        let result = file.find_signature_block().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_signature_block_empty_file() {
        let mut file = make_file(b"");
        let result = file.find_signature_block().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_signature_block_file_too_short() {
        // File shorter than minimum sig structure (prefix + 1 byte + suffix)
        let mut file = make_file(b"x");
        let result = file.find_signature_block().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_signature_block_works_on_bounded_file() {
        let body = b"bounded content";
        let mut bf =
            crate::io::BoundedFile::from_file(make_signed_file(body, VALID_SIG_B64)).unwrap();

        let result = bf.find_signature_block().unwrap();
        let loc = result.expect("expected Some(SigLocation)");
        assert_eq!(loc.content_len, body.len() as u64);
        assert_eq!(loc.sig_base64, VALID_SIG_B64.as_bytes());
    }

    #[test]
    fn test_find_signature_block_content_len_excludes_sig() {
        let body = b"0123456789";
        let mut file = make_signed_file(body, VALID_SIG_B64);

        let loc = file.find_signature_block().unwrap().unwrap();
        assert_eq!(loc.content_len, 10);
        assert_eq!(
            loc.file_len - loc.content_len,
            (SIG_V1_PREFIX.len() + VALID_SIG_B64.len() + SIG_SUFFIX.len()) as u64
        );
    }

    // ------------------------------------------------------------------
    // find_signature tests
    // ------------------------------------------------------------------

    #[test]
    fn test_find_signature_valid() {
        let mut file = make_signed_file(b"content", VALID_SIG_B64);

        match file.find_signature().unwrap() {
            FindSigResult::Found(loc) => {
                assert_eq!(loc.content_len, 7);
                assert_eq!(loc.sig_base64, VALID_SIG_B64.as_bytes());
            }
            _other => panic!("expected Found, got NotFound/Corrupt"),
        }
    }

    #[test]
    fn test_find_signature_corrupt_base64() {
        let mut file = make_signed_file(b"content", "!!!not-valid-base64!!!");

        assert!(matches!(
            file.find_signature().unwrap(),
            FindSigResult::Corrupt
        ));
    }

    #[test]
    fn test_find_signature_not_found() {
        let mut file = make_file(b"no signature here");

        assert!(matches!(
            file.find_signature().unwrap(),
            FindSigResult::NotFound
        ));
    }

    #[test]
    fn test_find_signature_not_found_empty() {
        let mut file = make_file(b"");

        assert!(matches!(
            file.find_signature().unwrap(),
            FindSigResult::NotFound
        ));
    }

    #[test]
    fn test_find_signature_prefix_only_no_suffix() {
        let mut contents = b"body".to_vec();
        contents.extend_from_slice(SIG_V1_PREFIX.as_bytes());
        contents.extend_from_slice(VALID_SIG_B64.as_bytes());
        let mut file = make_file(&contents);

        assert!(matches!(
            file.find_signature().unwrap(),
            FindSigResult::NotFound
        ));
    }

    #[test]
    fn test_find_signature_called_twice_is_idempotent() {
        let mut file = make_signed_file(b"content", VALID_SIG_B64);

        let first = match file.find_signature().unwrap() {
            FindSigResult::Found(loc) => loc.content_len,
            _ => panic!("expected Found"),
        };
        let second = match file.find_signature().unwrap() {
            FindSigResult::Found(loc) => loc.content_len,
            _ => panic!("expected Found"),
        };
        assert_eq!(first, second);
    }

    #[test]
    fn test_find_signature_body_containing_prefix_substring() {
        // Body contains something that looks like a prefix but isn't a full sig
        let mut body = b"some content with ".to_vec();
        body.extend_from_slice(&SIG_V1_PREFIX.as_bytes()[..5]);
        body.extend_from_slice(b" more content");
        let mut file = make_signed_file(&body, VALID_SIG_B64);

        match file.find_signature().unwrap() {
            FindSigResult::Found(loc) => {
                assert_eq!(loc.content_len, body.len() as u64);
            }
            _ => panic!("expected Found"),
        }
    }
}
