use crate::{cli::*, error::*, file::*, io::*};
use camino::Utf8PathBuf;
use std::fs::File;

fn needs_signing(path: &Utf8PathBuf, pubkeys: &PublicKeys) -> Result<bool, Err> {
    let file = File::open_ro(path)?;
    let mut bf = BoundedFile::from_file(file).loc(path)?;

    match bf.verify_sig(pubkeys) {
        Ok(()) => Ok(false),
        Err(
            AnonLocErr::SigCorrupt
            | AnonLocErr::SigInvalid
            | AnonLocErr::NoPublicKeys
            | AnonLocErr::SigMissing,
        ) => Ok(true),
        Err(e) => Err(e.loc(path)),
    }
}

pub fn sign(flags: CommonFlags, needed: bool, paths: Vec<Utf8PathBuf>) -> Result<String, Err> {
    let paths_to_sign: Vec<&Utf8PathBuf> = if needed {
        // `--needed` is defined in terms of whether the file currently verifies. Do not let the
        // global `--skip-verify` flag disable that check.
        let pubkeys = PublicKeys::from_root_path(&flags.root_dir)?;
        let mut paths_to_sign = Vec::new();
        for path in &paths {
            if needs_signing(path, &pubkeys)? {
                paths_to_sign.push(path);
            }
        }
        paths_to_sign
    } else {
        paths.iter().collect()
    };

    if paths_to_sign.is_empty() {
        return Ok(format!(
            "All {} files already had valid signatures",
            paths.len()
        ));
    }

    let privkey = PrivKey::from_common_flags(&flags)?;

    for path in &paths_to_sign {
        File::open_rw(path)?.sign(&privkey).loc(path)?;
    }

    if paths_to_sign.len() == 1 && paths.len() == 1 {
        Ok(format!("Signed {}", paths_to_sign[0]))
    } else if needed {
        Ok(format!(
            "Signed {} of {} files",
            paths_to_sign.len(),
            paths.len()
        ))
    } else if paths.len() == 1 {
        Ok(format!("Signed {}", paths[0]))
    } else {
        Ok(format!("Signed all {} files", paths.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::{PrivKey, PublicKeys};
    use crate::io::FileAux;
    use crate::testutil::unit_test_tmp_dir;
    use std::io::Write;

    #[test]
    fn needs_signing_true_for_unsigned_file() {
        let dir = unit_test_tmp_dir("sign_command", "needs_signing_true_for_unsigned_file");
        let path = dir.join("unsigned");
        std::fs::write(&path, b"unsigned").unwrap();

        assert!(needs_signing(&path, &PublicKeys::from_test_key()).unwrap());
    }

    #[test]
    fn needs_signing_false_for_valid_signature() {
        let dir = unit_test_tmp_dir("sign_command", "needs_signing_false_for_valid_signature");
        let path = dir.join("signed");
        std::fs::write(&path, b"signed").unwrap();

        let mut file = File::open_rw(&path).unwrap();
        file.sign(&PrivKey::from_test_key()).unwrap();

        assert!(!needs_signing(&path, &PublicKeys::from_test_key()).unwrap());
    }

    #[test]
    fn needs_signing_true_for_invalid_signature() {
        let dir = unit_test_tmp_dir("sign_command", "needs_signing_true_for_invalid_signature");
        let path = dir.join("invalid");
        std::fs::write(&path, b"invalid").unwrap();

        let mut file = File::open_rw(&path).unwrap();
        file.write_all(b"\n# bpt-sig-v1:corrupt+signature\n")
            .unwrap();

        assert!(needs_signing(&path, &PublicKeys::from_test_key()).unwrap());
    }
}
