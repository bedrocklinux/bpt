use crate::{error::*, make_field, marshalling::*, metadata::*, str::*};
use sha2::Digest;
use std::{borrow::Cow, fs::File};

/// A regular file in an installed package.
///
/// In addition to indicating that this is a regular file (rather than, for example, a symlink) this
/// field contains the file's checksum, sha256 serialized in base64.  The file's filename is
/// serialized/deserialized in a preceding [Filename] field rather than in this field directly.
#[derive(Clone, Debug, PartialEq)]
pub struct RegFile(Sha256);

pub type Sha256 = [u8; 256 / 8];

make_field!(RegFile, InstPkgKey);

impl RegFile {
    pub fn from_sha256(sha256: Sha256) -> Self {
        Self(sha256)
    }

    pub fn from_file(file: &mut File) -> Result<Self, AnonLocErr> {
        let mut hasher = sha2::Sha256::new();
        std::io::copy(file, &mut hasher).map_err(AnonLocErr::Read)?;
        let sha256: Sha256 = hasher.finalize().into();
        Ok(Self::from_sha256(sha256))
    }
}

impl FromFieldStr for RegFile {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        value
            .as_bytes()
            .base64_decode()?
            .try_into()
            .map_err(|_| {
                AnonLocErr::FieldInvalid(Self::NAME, "decodes to incorrect length".to_string())
            })
            .map(Self::from_sha256)
    }
}

impl AsBytes for RegFile {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.base64_encode().into_bytes())
    }
}

impl std::fmt::Display for RegFile {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0.base64_encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<RegFile, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(RegFile::NAME))
            .and_then(RegFile::from_field_str)
    }

    #[test]
    fn roundtrip() {
        let sha: Sha256 = [0xab; 32];
        let encoded = sha.base64_encode();
        let regfile = parse(&encoded).unwrap();
        assert_eq!(std::str::from_utf8(&regfile.as_bytes()).unwrap(), encoded);
    }

    #[test]
    fn wrong_length() {
        // 16 bytes encoded in base64 — valid base64 but wrong length for SHA-256
        let short: [u8; 16] = [0xcd; 16];
        let encoded = short.base64_encode();
        assert!(parse(&encoded).is_err());
    }

    #[test]
    fn invalid_base64() {
        assert!(parse("not valid base64!!!").is_err());
    }

    #[test]
    fn empty() {
        assert!(parse("").is_err());
    }
}
