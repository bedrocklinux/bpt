use crate::{cli::CommonFlags, constant::*, error::*, file::sig::*, str::*};
use camino::Utf8Path;
use nix::sys::termios::{LocalFlags, SetArg, Termios, tcgetattr, tcsetattr};
use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom, Write, stdout},
    os::fd::{AsRawFd, OwnedFd},
};
use zeroize::Zeroizing;

/// Minisign secret key used to verify various bpt files
pub enum PrivKey {
    SkipSign,
    SignWithKey(minisign::SecretKey),
}

impl PrivKey {
    pub fn from_path(path: &Utf8Path, passphrase_file: Option<&Utf8Path>) -> Result<Self, Err> {
        let pass = read_secret(path, passphrase_file)?;

        print!("Decrypting secret key... ");
        stdout().flush().map_err(Err::FlushStdout)?;

        // minisign's API takes a plain String password, creating an unzeroized copy.
        // This is a limitation of the minisign crate; fixing it requires an upstream change.
        match minisign::SecretKey::from_file(path, Some(pass.to_string())) {
            Ok(key) => {
                println!("done");
                Ok(Self::SignWithKey(key))
            }
            Err(e) => {
                println!("error!");
                Err(Err::LoadSecretKey(path.to_owned(), e.to_string()))
            }
        }
    }

    fn from_skipping_signing() -> Self {
        Self::SkipSign
    }

    pub fn from_common_flags(flags: &CommonFlags) -> Result<Self, Err> {
        if flags.skip_sign {
            Ok(Self::from_skipping_signing())
        } else {
            Self::from_path(&flags.priv_key, flags.priv_key_passphrase_file.as_deref())
        }
    }
}

fn read_secret(
    priv_key_path: &Utf8Path,
    passphrase_file: Option<&Utf8Path>,
) -> Result<Zeroizing<String>, Err> {
    if let Some(path) = passphrase_file {
        let mut s = Zeroizing::new(
            std::fs::read_to_string(path).map_err(|e| Err::Read(path.to_string(), e))?,
        );
        let trimmed_len = s.trim_end_matches(['\n', '\r']).len();
        s.truncate(trimmed_len);

        if s.is_empty() {
            return Err(Err::Read(
                path.to_string(),
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Passphrase file is empty"),
            ));
        }

        return Ok(s);
    }

    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err(Err::Read(
            priv_key_path.to_string(),
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Non-interactive input detected; provide --priv-key-passphrase-file",
            ),
        ));
    }

    DisableEcho::new("minisign secret key password: ")?.read_secret()
}

pub trait Sign {
    fn sign(&mut self, privkey: &PrivKey) -> Result<(), AnonLocErr>
    where
        Self: std::marker::Sized;

    fn strip_signature(&mut self) -> Result<(), AnonLocErr>
    where
        Self: std::marker::Sized;
}

impl Sign for File {
    fn sign(&mut self, privkey: &PrivKey) -> Result<(), AnonLocErr> {
        let seckey = match &privkey {
            PrivKey::SkipSign => return Ok(()),
            PrivKey::SignWithKey(seckey) => seckey,
        };

        // If the file is already signed, strip existing signature before adding new one.
        self.strip_signature()?;

        // Get base64 encoded minisign signature without minisign comments
        self.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
        let bones: minisign::SignatureBones = minisign::sign(None, seckey, &mut *self, None, None)
            .map_err(|e| AnonLocErr::CouldNotSign(e.to_string()))?
            .into();
        let bytes = bones.to_bytes();

        // Format with v1 prefix for minisign signatures
        let sig = format!("{}{}{}", SIG_V1_PREFIX, &bytes.base64_encode(), SIG_SUFFIX);

        // Append
        self.seek(SeekFrom::End(0)).map_err(AnonLocErr::Seek)?;
        self.write_all(sig.as_bytes()).map_err(AnonLocErr::Write)?;

        Ok(())
    }

    fn strip_signature(&mut self) -> Result<(), AnonLocErr> {
        use super::sig::FindSigResult;
        let sig_loc = match self.find_signature()? {
            FindSigResult::Found(loc) => loc,
            FindSigResult::Corrupt => {
                // Intentionally treat a trailing signature-shaped block as replaceable
                // during re-signing, even if malformed, so stale/corrupt trailers are
                // removed before appending the new signature.
                let Some(loc) = self.find_signature_block()? else {
                    return Ok(());
                };
                loc
            }
            FindSigResult::NotFound => return Ok(()), // not signed
        };

        self.set_len(sig_loc.content_len)
            .map_err(AnonLocErr::Truncate)
    }
}

/// Disables terminal echo on creation and resets terminal settings on drop.  Used for reading
/// passwords.
struct DisableEcho {
    terminal_fd: OwnedFd,
    original_settings: Termios,
}

impl DisableEcho {
    fn new(prompt: &str) -> Result<Self, Err> {
        let terminal = std::fs::OpenOptions::new()
            .read(true)
            .write(false)
            .open(TTY_PATH)
            .map_err(|e| Err::Open(TTY_PATH.to_owned(), e))?;
        let terminal_fd: OwnedFd = terminal.into();
        let original_settings = tcgetattr(terminal_fd.as_raw_fd())
            .map_err(|e| Err::Read(TTY_PATH.to_owned(), e.into()))?;

        let mut new_settings = original_settings.clone();
        new_settings.local_flags &= !LocalFlags::ECHO;
        new_settings.local_flags |= LocalFlags::ECHONL;
        tcsetattr(terminal_fd.as_raw_fd(), SetArg::TCSANOW, &new_settings)
            .map_err(|e| Err::Write(TTY_PATH.to_owned(), e.into()))?;

        print!("{prompt}");
        Write::flush(&mut stdout()).map_err(Err::FlushStdout)?;
        Ok(Self {
            terminal_fd,
            original_settings,
        })
    }

    fn read_secret(&self) -> Result<Zeroizing<String>, Err> {
        // Read from the tty fd rather than stdin, so that password input comes
        // from the terminal even if stdin is redirected.
        let mut reader = BufReader::new(File::from(
            self.terminal_fd
                .try_clone()
                .map_err(|e| Err::Read(TTY_PATH.to_owned(), e))?,
        ));
        let mut s = String::new();
        reader
            .read_line(&mut s)
            .map_err(|e| Err::Read(TTY_PATH.to_owned(), e))?;
        match s.strip_suffix('\n') {
            Some(s) => Ok(Zeroizing::new(s.to_owned())),
            None => Err(Err::Read(
                TTY_PATH.to_owned(),
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "No trailing newline"),
            )),
        }
    }
}

impl Drop for DisableEcho {
    fn drop(&mut self) {
        let _ = tcsetattr(
            self.terminal_fd.as_raw_fd(),
            SetArg::TCSANOW,
            &self.original_settings,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AnonLocErr;
    use crate::file::{PublicKeys, VerifySignature};
    use crate::io::*;
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;
    use std::io::{Read, Write};

    impl PrivKey {
        pub fn from_test_key() -> PrivKey {
            let bytes = include_bytes!("../../tests/keys/test-key-password-is-bpt.decrypted");
            let minisign_key = minisign::SecretKey::from_bytes(bytes).unwrap();
            PrivKey::SignWithKey(minisign_key)
        }
    }

    fn read_file(file: &mut File) -> Vec<u8> {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        buf
    }

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("privkey", name)
    }

    fn write_passphrase_file(dir: &Utf8Path, contents: &str) -> Utf8PathBuf {
        let path = dir.join("passphrase");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_read_secret_trims_trailing_newlines() {
        let dir = test_dir("trims_trailing_newlines");
        let path = write_passphrase_file(&dir, "secret\n");
        let result = read_secret(&path, Some(&path)).unwrap();
        assert_eq!(&*result, "secret");
    }

    #[test]
    fn test_read_secret_trims_crlf() {
        let dir = test_dir("trims_crlf");
        let path = write_passphrase_file(&dir, "secret\r\n");
        let result = read_secret(&path, Some(&path)).unwrap();
        assert_eq!(&*result, "secret");
    }

    #[test]
    fn test_read_secret_trims_multiple_trailing_newlines() {
        let dir = test_dir("trims_multiple_trailing_newlines");
        let path = write_passphrase_file(&dir, "secret\n\r\n\n");
        let result = read_secret(&path, Some(&path)).unwrap();
        assert_eq!(&*result, "secret");
    }

    #[test]
    fn test_read_secret_no_trailing_newline() {
        let dir = test_dir("no_trailing_newline");
        let path = write_passphrase_file(&dir, "secret");
        let result = read_secret(&path, Some(&path)).unwrap();
        assert_eq!(&*result, "secret");
    }

    #[test]
    fn test_read_secret_empty_file() {
        let dir = test_dir("empty_file");
        let path = write_passphrase_file(&dir, "");
        assert!(read_secret(&path, Some(&path)).is_err());
    }

    #[test]
    fn test_read_secret_only_newlines() {
        let dir = test_dir("only_newlines");
        let path = write_passphrase_file(&dir, "\n\r\n");
        assert!(read_secret(&path, Some(&path)).is_err());
    }

    #[test]
    fn test_sign() {
        let contents = b"Test file contents";

        let privkey = PrivKey::from_test_key();
        let mut file = File::create_memfd(c"file-name", contents).unwrap();
        file.sign(&privkey).unwrap();

        let contents_with_sig = read_file(&mut file);
        assert!(contents_with_sig.len() > contents.len());
        assert!(contents_with_sig.starts_with(contents));

        let pubkeys = PublicKeys::from_test_key();
        let mut bf = BoundedFile::from_file(file).unwrap();
        assert!(bf.verify_sig(&pubkeys).is_ok());
    }

    #[test]
    fn test_sign_already_signed() {
        let contents = b"Test file contents";

        let privkey = PrivKey::from_test_key();
        let mut file = File::create_memfd(c"file-name", contents).unwrap();
        let old_sig = b"\n# bpt-sig-v1:RUSWg+V4uzz1zRLiMvYdSiKjPd86/ZZC8TYnsmwrPsYTr2NUmnG5fN+sHoLg90YU2tNXtYscxROVXgYh+O/L/R4/Z3wZKhjZ8QA\n";
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(old_sig).unwrap();

        file.sign(&privkey).unwrap();

        // Check that the old sig is no longer in the file
        let contents_with_sig = read_file(&mut file);
        assert!(contents_with_sig.starts_with(contents));
        assert!(
            !contents_with_sig
                .windows(old_sig.len())
                .any(|window| window == old_sig)
        );

        // Validate the new signature
        let pubkeys = PublicKeys::from_test_key();
        let mut bf = BoundedFile::from_file(file).unwrap();
        assert!(bf.verify_sig(&pubkeys).is_ok());
    }

    #[test]
    fn test_sign_with_corrupt_trailing_sig_block() {
        let contents = b"Test file contents";

        let privkey = PrivKey::from_test_key();
        let mut file = File::create_memfd(c"file-name", contents).unwrap();
        let corrupt_sig = b"\n# bpt-sig-v1:corrupt-signature-block\n";
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(corrupt_sig).unwrap();

        file.sign(&privkey).unwrap();

        // Re-signing should strip trailing signature-like blocks first, even if malformed.
        let contents_with_sig = read_file(&mut file);
        assert!(contents_with_sig.starts_with(contents));
        assert!(
            !contents_with_sig
                .windows(corrupt_sig.len())
                .any(|window| window == corrupt_sig)
        );

        let pubkeys = PublicKeys::from_test_key();
        let mut bf = BoundedFile::from_file(file).unwrap();
        assert!(bf.verify_sig(&pubkeys).is_ok());
    }

    #[test]
    fn test_strip_signature() {
        let contents = b"Test file contents";

        let mut file = File::create_memfd(c"file-name", contents).unwrap();
        let old_sig = b"\n# bpt-sig-v1:RUSWg+V4uzz1zRLiMvYdSiKjPd86/ZZC8TYnsmwrPsYTr2NUmnG5fN+sHoLg90YU2tNXtYscxROVXgYh+O/L/R4/Z3wZKhjZ8QA\n";
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(old_sig).unwrap();

        file.strip_signature().unwrap();

        // Check that the old sig is no longer in the file
        let contents_with_sig = read_file(&mut file);
        assert_eq!(&contents_with_sig, contents);
        assert!(
            !contents_with_sig
                .windows(old_sig.len())
                .any(|window| window == old_sig)
        );

        // Confirm no signature
        let pubkeys = PublicKeys::from_test_key();
        let mut bf = BoundedFile::from_file(file).unwrap();
        assert!(matches!(
            bf.verify_sig(&pubkeys),
            Err(AnonLocErr::SigMissing)
        ));
    }
}
