use crate::error::*;
use crate::str::absolute_path_from_cwd;
use camino::{Utf8Path, Utf8PathBuf};
use std::io::ErrorKind;

/// Some file type constructors take the path to the file, while others take the root directory of
/// the file system tree that contains the file and append the per-file-type path appropriately.
///
/// Using the same path type for both classes of constructors is error prone.  Thus, this is a
/// light wrapper around an expected root directory to minimize the possibility of such errors.
#[derive(Clone)]
pub struct RootDir(Utf8PathBuf);

impl RootDir {
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    pub fn adjust_bedrock_prefix(&self) -> Result<Utf8PathBuf, Err> {
        let output = std::process::Command::new("brl")
            .arg("which")
            .arg(self.as_path())
            .output();

        match output {
            // Not running on Bedrock
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(self.as_path().into()),
            Err(e) => Err(Err::RunBrlWhich(self.to_string(), e)),
            Ok(output) if !output.status.success() => Err(Err::RunBrlWhich(
                self.to_string(),
                std::io::Error::other(format!("brl which failed with status: {}", output.status)),
            )),
            Ok(output) => {
                let stratum = String::from_utf8(output.stdout)
                    .map_err(|_| Err::BrlWhichNonUtf8(self.to_string()))?;
                let stratum = stratum.trim_end(); // remove trailing newline
                if stratum == "global" {
                    Ok(self.as_path().into())
                } else {
                    Ok(Utf8PathBuf::from(format!("/bedrock/strata/{stratum}")))
                }
            }
        }
    }

    pub fn from_path(path: &Utf8Path) -> Self {
        Self(path.into())
    }
}

impl std::str::FromStr for RootDir {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = Utf8PathBuf::from(s);
        let path = absolute_path_from_cwd(path.as_path()).map_err(|e| {
            if e.kind() == ErrorKind::InvalidData {
                Err::InputFieldInvalid("root directory", e.to_string())
            } else {
                Err::Open("current working directory".to_string(), e)
            }
        })?;
        Ok(RootDir(path))
    }
}

impl std::fmt::Display for RootDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
