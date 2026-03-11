use crate::{error::*, file::BptConf, io::ProcessCredentials};
use nix::unistd::{Uid, geteuid};
use std::cell::RefCell;

/// Lazily resolve the credentials needed to source `.bbuild` files.
///
/// Many commands can operate entirely on binary packages and should not fail early on unrelated
/// `[build]` configuration problems. Delay validation until a code path actually needs to source a
/// build definition.
pub struct QueryCredentials<'a> {
    bpt_conf: &'a BptConf,
    euid: Uid,
    credentials: RefCell<Option<Option<ProcessCredentials>>>,
}

impl<'a> QueryCredentials<'a> {
    pub fn new(bpt_conf: &'a BptConf) -> Self {
        Self::new_for_euid(bpt_conf, geteuid())
    }

    pub(crate) fn new_for_euid(bpt_conf: &'a BptConf, euid: Uid) -> Self {
        Self {
            bpt_conf,
            euid,
            credentials: RefCell::new(None),
        }
    }

    pub fn get(&self) -> Result<Option<ProcessCredentials>, Err> {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            return Ok(credentials.clone());
        }

        let credentials = self.bpt_conf.build_credentials_for_euid(self.euid)?;
        *self.credentials.borrow_mut() = Some(credentials.clone());
        Ok(credentials)
    }
}
