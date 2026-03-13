use crate::file::sig::*;
use crate::{cli::CommonFlags, constant::*, error::*, io::*, location::*, str::Base64Decode};
use minisign::{SignatureBones, SignatureBox};
use std::fs::File;
use std::io::{ErrorKind, Seek, SeekFrom};

/// Public keys used to verify various bpt files.
/// Supports mixed key formats detected by file content.
pub enum PublicKeys {
    SkipVerify,
    VerifyWithKeys {
        /// v1 (minisign Ed25519) public keys
        v1_keys: Vec<minisign::PublicKey>,
        // Future: v2_keys: Vec<...>,
    },
}

impl PublicKeys {
    pub fn from_root_path(root: &RootDir) -> Result<Self, Err> {
        match root.as_path().join(PUB_KEY_DIR_PATH).readdir() {
            Ok(paths) => {
                let mut v1_keys = Vec::new();
                for path in paths {
                    let content = std::fs::read_to_string(&path)
                        .map_err(|e| Err::Read(path.to_string(), e))?;
                    if content.starts_with("untrusted comment:") {
                        // v1: minisign key format
                        let key = minisign::PublicKey::from_file(&path)
                            .map_err(|e| Err::LoadPublicKey(path, e.to_string()))?;
                        v1_keys.push(key);
                    } else {
                        return Err(Err::UnrecognizedKeyFormat(path));
                    }
                }
                Ok(Self::VerifyWithKeys { v1_keys })
            }
            Err(Err::ReadDir(_, e)) if e.kind() == ErrorKind::NotFound => {
                Ok(Self::VerifyWithKeys {
                    v1_keys: Vec::new(),
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn from_skipping_verification() -> Self {
        Self::SkipVerify
    }

    pub fn from_common_flags(flags: &CommonFlags) -> Result<Self, Err> {
        if flags.skip_verify {
            Ok(Self::from_skipping_verification())
        } else {
            Self::from_root_path(&flags.root_dir)
        }
    }
}

/// Verify a v1 (minisign Ed25519) signature against the provided keys.
fn verify_v1(
    mut file: BoundedFile,
    sig_loc: &SigLocation,
    keys: &[minisign::PublicKey],
    sig_size: u64,
    strip_sig: bool,
) -> Result<BoundedFile, AnonLocErr> {
    // Decode signature
    let sig_decoded = sig_loc
        .sig_base64
        .base64_decode()
        .map_err(|_| AnonLocErr::SigCorrupt)?;
    let sig_bones = SignatureBones::from_bytes(&sig_decoded).map_err(|_| AnonLocErr::SigCorrupt)?;
    let sig_box: SignatureBox = sig_bones.into();

    file.decrease_upper_bound_by(sig_size)?;

    // Verify signature against each key
    for key in keys {
        file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
        if minisign::verify(key, &sig_box, &mut file, true, false, false).is_ok() {
            if !strip_sig {
                file.increase_upper_bound_by(sig_size)?;
            }
            file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
            return Ok(file);
        }
    }

    Err(AnonLocErr::SigInvalid)
}

pub trait VerifySignature {
    fn verify_sig(self, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr>
    where
        Self: Sized;

    fn verify_and_strip_sig(self, pubkeys: &PublicKeys) -> Result<BoundedFile, AnonLocErr>
    where
        Self: Sized;
}

impl VerifySignature for File {
    fn verify_sig(self, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr> {
        BoundedFile::from_file(self)?
            .verify_sig(pubkeys)
            .map(BoundedFile::into_inner)
    }

    fn verify_and_strip_sig(self, pubkeys: &PublicKeys) -> Result<BoundedFile, AnonLocErr> {
        BoundedFile::from_file(self)?.verify_and_strip_sig(pubkeys)
    }
}

impl VerifySignature for BoundedFile {
    fn verify_sig(mut self, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr> {
        let sig_loc = match (self.find_signature()?, pubkeys) {
            (FindSigResult::Found(loc), _) => loc,
            (FindSigResult::Corrupt, _) => return Err(AnonLocErr::SigCorrupt),
            (FindSigResult::NotFound, PublicKeys::SkipVerify) => {
                self.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
                return Ok(self);
            }
            (FindSigResult::NotFound, PublicKeys::VerifyWithKeys { .. }) => {
                return Err(AnonLocErr::SigMissing);
            }
        };

        let keys = match (&sig_loc.format, pubkeys) {
            (_, PublicKeys::SkipVerify) => {
                self.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
                return Ok(self);
            }
            (SigFormat::V1, PublicKeys::VerifyWithKeys { v1_keys, .. }) => v1_keys,
        };
        if keys.is_empty() {
            return Err(AnonLocErr::NoPublicKeys);
        }

        match sig_loc.format {
            SigFormat::V1 => {
                let sig_size = sig_loc.file_len - sig_loc.content_len;
                let mut verified = verify_v1(self, &sig_loc, keys, sig_size, false)?;
                verified
                    .seek(SeekFrom::Start(0))
                    .map_err(AnonLocErr::Seek)?;
                Ok(verified)
            }
        }
    }

    fn verify_and_strip_sig(self, pubkeys: &PublicKeys) -> Result<BoundedFile, AnonLocErr> {
        let mut file = self;
        let sig_loc = match (file.find_signature()?, pubkeys) {
            (FindSigResult::Found(loc), _) => loc,
            (FindSigResult::Corrupt, _) => return Err(AnonLocErr::SigCorrupt),
            (FindSigResult::NotFound, PublicKeys::SkipVerify) => {
                file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
                return Ok(file);
            }
            (FindSigResult::NotFound, PublicKeys::VerifyWithKeys { .. }) => {
                return Err(AnonLocErr::SigMissing);
            }
        };

        let sig_size = sig_loc.file_len - sig_loc.content_len;
        let keys = match (&sig_loc.format, pubkeys) {
            (_, PublicKeys::SkipVerify) => {
                file.decrease_upper_bound_by(sig_size)?;
                file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
                return Ok(file);
            }
            (SigFormat::V1, PublicKeys::VerifyWithKeys { v1_keys, .. }) => v1_keys,
        };
        if keys.is_empty() {
            return Err(AnonLocErr::NoPublicKeys);
        }

        match sig_loc.format {
            SigFormat::V1 => verify_v1(file, &sig_loc, keys, sig_size, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AnonLocErr;
    use crate::file::PrivKey;
    use crate::file::privkey::Sign;
    use crate::io::FileAux;
    use crate::location::RootDir;
    use crate::testutil::unit_test_tmp_dir;
    use std::io::{Read, Write};

    impl PublicKeys {
        pub fn from_test_key() -> PublicKeys {
            let bytes = include_bytes!("../../tests/keys/test-key-password-is-bpt.pub");
            let string = std::str::from_utf8(bytes).unwrap();
            let keybox = minisign::PublicKeyBox::from_string(string).unwrap();
            let minisign_key = minisign::PublicKey::from_box(keybox).unwrap();

            PublicKeys::VerifyWithKeys {
                v1_keys: vec![minisign_key],
            }
        }
    }

    fn create_unsigned_file(contents: &[u8]) -> BoundedFile {
        BoundedFile::from_file(File::create_memfd(c"file-name", contents).unwrap()).unwrap()
    }

    fn create_unsigned_raw_file(contents: &[u8]) -> File {
        File::create_memfd(c"file-name", contents).unwrap()
    }

    fn create_signed_raw_file(contents: &[u8]) -> File {
        let mut file = File::create_memfd(c"file-name", contents).unwrap();
        file.sign(&PrivKey::from_test_key()).unwrap();
        file
    }

    fn create_signed_file(contents: &[u8]) -> BoundedFile {
        BoundedFile::from_file(create_signed_raw_file(contents)).unwrap()
    }

    #[test]
    fn test_from_root_path_missing_key_dir_returns_empty_keyset() {
        let tmp = unit_test_tmp_dir(
            "pubkeys",
            "test_from_root_path_missing_key_dir_returns_empty_keyset",
        );
        let root = RootDir::from_path(&tmp);

        match PublicKeys::from_root_path(&root).unwrap() {
            PublicKeys::VerifyWithKeys { v1_keys } => assert!(v1_keys.is_empty()),
            PublicKeys::SkipVerify => panic!("expected verification mode with empty key set"),
        }
    }

    fn read_bounded_file(file: &mut BoundedFile) -> Vec<u8> {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        buf
    }

    #[test]
    fn test_verify_sig_valid() {
        let contents = b"Test file contents";
        let mut file = create_signed_file(contents);
        let contents_with_sig = read_bounded_file(&mut file);

        let pubkeys = PublicKeys::from_test_key();
        let mut file = file.verify_sig(&pubkeys).unwrap();

        // Check that signature is not stripped
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, contents_with_sig);
    }

    #[test]
    fn test_verify_sig_valid_file() {
        let contents = b"Test file contents";
        let mut file = create_signed_raw_file(contents);
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut contents_with_sig = Vec::new();
        file.read_to_end(&mut contents_with_sig).unwrap();

        let pubkeys = PublicKeys::from_test_key();
        let mut file = file.verify_sig(&pubkeys).unwrap();

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, contents_with_sig);
    }

    #[test]
    fn test_verify_sig_invalid() {
        let contents = b"Test file contents";
        let mut file = create_unsigned_raw_file(contents);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all("\n# bpt-sig-v1:RUSWg+V4uzz1zRLiMvYdSiKjPd86/ZZC8TYnsmwrPsYTr2NUmnG5fN+sHoLg90YU2tNXtYscxROVXgYh+O/L/R4/Z3wZKhjZ8QA\n".as_bytes()).unwrap();

        let pubkeys = PublicKeys::from_test_key();
        let bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_sig(&pubkeys),
            Err(AnonLocErr::SigInvalid)
        ));
    }

    #[test]
    fn test_verify_sig_corrupt() {
        let contents = b"Test file contents";
        let mut file = create_unsigned_raw_file(contents);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all("\n# bpt-sig-v1:corrupt-signature-block\n".as_bytes())
            .unwrap();

        let pubkeys = PublicKeys::from_test_key();
        let bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_sig(&pubkeys),
            Err(AnonLocErr::SigCorrupt)
        ));
    }

    #[test]
    fn test_verify_sig_missing() {
        let contents = b"Test file contents";
        let file = create_unsigned_file(contents);

        let pubkeys = PublicKeys::from_test_key();
        assert!(matches!(
            file.verify_sig(&pubkeys),
            Err(AnonLocErr::SigMissing)
        ));
    }

    #[test]
    fn test_verify_sig_no_public_keys() {
        let contents = b"Test file contents";
        let file = create_signed_file(contents);

        let pubkeys = PublicKeys::VerifyWithKeys {
            v1_keys: Vec::new(),
        };
        assert!(matches!(
            file.verify_sig(&pubkeys),
            Err(AnonLocErr::NoPublicKeys)
        ));
    }

    #[test]
    fn test_verify_and_strip_sig_valid() {
        let contents = b"Test file contents";
        let file = create_signed_file(contents);

        let pubkeys = PublicKeys::from_test_key();
        let mut stripped = file.verify_and_strip_sig(&pubkeys).unwrap();

        let mut buf = Vec::new();
        stripped.read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len(), contents.len());
    }

    #[test]
    fn test_verify_and_strip_sig_invalid() {
        let contents = b"Test file contents";
        let mut file = create_unsigned_raw_file(contents);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all("\n# bpt-sig-v1:RUSWg+V4uzz1zRLiMvYdSiKjPd86/ZZC8TYnsmwrPsYTr2NUmnG5fN+sHoLg90YU2tNXtYscxROVXgYh+O/L/R4/Z3wZKhjZ8QA\n".as_bytes()).unwrap();

        let pubkeys = PublicKeys::from_test_key();
        let bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_and_strip_sig(&pubkeys),
            Err(AnonLocErr::SigInvalid)
        ));
    }

    #[test]
    fn test_verify_and_strip_sig_corrupt() {
        let contents = b"Test file contents";
        let mut file = create_unsigned_raw_file(contents);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all("\n# bpt-sig-v1:corrupt-signature-block\n".as_bytes())
            .unwrap();

        let pubkeys = PublicKeys::from_test_key();
        let bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_and_strip_sig(&pubkeys),
            Err(AnonLocErr::SigCorrupt)
        ));
    }

    #[test]
    fn test_verify_and_strip_sig_missing() {
        let contents = b"Test file contents";
        let file = create_unsigned_file(contents);

        let pubkeys = PublicKeys::from_test_key();
        assert!(matches!(
            file.verify_and_strip_sig(&pubkeys),
            Err(AnonLocErr::SigMissing)
        ));
    }

    #[test]
    fn test_verify_and_strip_sig_no_public_keys() {
        let contents = b"Test file contents";
        let file = create_signed_file(contents);

        let pubkeys = PublicKeys::VerifyWithKeys {
            v1_keys: Vec::new(),
        };
        assert!(matches!(
            file.verify_and_strip_sig(&pubkeys),
            Err(AnonLocErr::NoPublicKeys)
        ));
    }

    #[test]
    fn test_verify_sig_skip_verify_corrupt_trailer_errors() {
        let contents = b"Test file contents";
        let mut file = create_unsigned_raw_file(contents);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all("\n# bpt-sig-v1:corrupt-signature-block\n".as_bytes())
            .unwrap();

        let pubkeys = PublicKeys::from_skipping_verification();
        let bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_sig(&pubkeys),
            Err(AnonLocErr::SigCorrupt)
        ));
    }

    #[test]
    fn test_verify_sig_skip_verify_signed_file_returns_full_file() {
        let contents = b"Test file contents";
        let mut file = create_signed_file(contents);
        let contents_with_sig = read_bounded_file(&mut file);

        let pubkeys = PublicKeys::from_skipping_verification();
        let mut file = file.verify_sig(&pubkeys).unwrap();

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, contents_with_sig);
    }

    #[test]
    fn test_verify_and_strip_sig_skip_verify_signed_file_strips_sig() {
        let contents = b"Test file contents";
        let file = create_signed_file(contents);

        let pubkeys = PublicKeys::from_skipping_verification();
        let mut stripped = file.verify_and_strip_sig(&pubkeys).unwrap();

        let mut buf = Vec::new();
        stripped.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, contents);
    }

    #[test]
    fn test_public_keys_from_root_path_mixed_files_fails() {
        let tmp = unit_test_tmp_dir("pubkeys", "mixed_files_fails");
        let root = RootDir::from_path(&tmp);

        let keydir = root.as_path().join(PUB_KEY_DIR_PATH);
        std::fs::create_dir_all(&keydir).unwrap();
        let valid_key = keydir.join("valid.pub");
        std::fs::write(
            &valid_key,
            include_bytes!("../../tests/keys/test-key-password-is-bpt.pub"),
        )
        .unwrap();
        let invalid_key = keydir.join("not-a-key.txt");
        std::fs::write(&invalid_key, b"definitely not a minisign key").unwrap();

        let result = PublicKeys::from_root_path(&root);

        assert!(matches!(result, Err(Err::UnrecognizedKeyFormat(path)) if path == invalid_key));
    }
}
