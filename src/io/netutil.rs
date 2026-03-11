use crate::{
    color::*,
    error::*,
    file::{BptConf, NetworkingUtil},
    io::{is_executable_in_paths, path_env},
    location::Url,
};
use std::{
    fs::File,
    io::Seek,
    os::fd::{AsFd, AsRawFd, FromRawFd},
    process::Stdio,
};

/// Networking utility
pub struct NetUtil<'a> {
    print_stderr: bool,
    /// [NetUtil] is often created preemptively, in case it is needed, but may never actually be
    /// used.  The inability to find a networking utility at creation may be acceptable, and so
    /// delay the error until the [NetUtil] is actually needed.
    util: Option<&'a NetworkingUtil>,
}

impl<'a> NetUtil<'a> {
    pub fn new(bpt_conf: &'a BptConf, print_stderr: bool) -> Self {
        let paths = path_env();

        for util in bpt_conf.networking.utils.iter() {
            let mut args = util.as_slice().iter().map(|s| s.as_str());

            let Some(exe) = args.next() else {
                continue;
            };
            if !is_executable_in_paths(exe, &paths) {
                continue;
            };

            return Self {
                print_stderr: print_stderr || bpt_conf.networking.print_stderr,
                util: Some(util),
            };
        }

        Self {
            print_stderr: print_stderr || bpt_conf.networking.print_stderr,
            util: None,
        }
    }

    pub fn download(&self, url: &Url, file: &mut File) -> Result<(), Err> {
        let Some(util) = self.util else {
            return Err(Err::NoNetUtilInPath);
        };

        let mut args = util.as_slice().iter().map(|s| s.as_str());
        let exe = args.next().ok_or_else(|| Err::NoNetUtilInPath)?;
        let mut cmd = std::process::Command::new(exe);

        // Redirect the utility's stdout to the output file.
        //
        // Rust's interface for redirecting command output to a file seems to insist on consuming
        // the file such any following command would have to re-open it.  It does not offer things
        // like `&mut File`, only `File`.  Work around this by duping the file descriptor.
        let fd: std::os::fd::BorrowedFd = file.as_fd();
        let dup_fd = nix::unistd::dup(fd.as_raw_fd()).map_err(Err::Dup)?;
        // Safety: We just duped this file, and so know the file descriptor is valid.
        let io = unsafe { std::process::Stdio::from_raw_fd(dup_fd) };
        cmd.stdout(io);

        if self.print_stderr {
            eprint!("{}> {exe}", Color::Deemphasize);
        } else {
            cmd.stderr(Stdio::null());
        }
        // NetworkingUtil enforces existence of `{}` substitution variable; no need to check if it
        // doesn't exist and error here.
        for arg in args {
            let term = if arg == "{}" { url.as_str() } else { arg };
            if self.print_stderr {
                eprint!(" {term}");
            }
            cmd.arg(term);
        }
        if self.print_stderr {
            eprint!("{}", Color::Default);
            eprintln!();
        }

        let status = cmd
            .status()
            .map_err(|e| Err::NetUtilError(exe.to_owned(), e))?;

        if !status.success() {
            return Err(Err::NetUtilNonZero(
                exe.to_owned(),
                status.code().unwrap_or(1),
            ));
        }

        file.rewind()
            .map_err(|e| Err::Seek("<downloaded-file>".to_owned(), e))?;
        Ok(())
    }
}
