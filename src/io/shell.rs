use crate::{constant::*, error::*, io::*, str::*};
use camino::Utf8Path;
use nix::{
    libc::{PIPE_BUF, STDERR_FILENO, STDOUT_FILENO},
    poll::{PollFd, PollFlags, poll},
    sys::wait::{WaitStatus, waitpid},
    unistd::{ForkResult, Gid, Uid, close, dup2, execvp, fork, pipe, read, setgid, setuid},
};
use std::fs::File;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    io::Write,
    io::{ErrorKind, Seek, SeekFrom},
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
    process::exit,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessCredentials {
    pub user_name: String,
    pub uid: Uid,
    pub gid: Gid,
}

fn drop_process_credentials(creds: &ProcessCredentials) -> Result<(), AnonLocErr> {
    let user_name = CString::new(creds.user_name.as_str()).map_err(|_| {
        AnonLocErr::DropPrivileges(std::io::Error::new(
            ErrorKind::InvalidInput,
            "configured username contains NUL byte",
        ))
    })?;

    // Safety: forked child is single-threaded here, and initgroups only borrows the CString.
    if unsafe { nix::libc::initgroups(user_name.as_ptr(), creds.gid.as_raw()) } != 0 {
        return Err(AnonLocErr::DropPrivileges(std::io::Error::last_os_error()));
    }
    setgid(creds.gid).map_err(|e| AnonLocErr::DropPrivileges(e.into()))?;
    setuid(creds.uid).map_err(|e| AnonLocErr::DropPrivileges(e.into()))?;

    Ok(())
}

fn append_inline_shell_scripts(
    cmd: &mut String,
    script_fds: &[BorrowedFd],
) -> Result<(), AnonLocErr> {
    for fd in script_fds {
        // `dup()` shares the underlying file description, so restore the original offset after
        // reading to avoid mutating the caller-visible fd state.
        let fd = nix::unistd::dup(fd.as_raw_fd()).map_err(AnonLocErr::Dup)?;
        // Safety: `dup()` returned a fresh owned fd.
        let mut file = File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        let pos = file.stream_position().map_err(AnonLocErr::Seek)?;
        file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
        let script = file.read_small_file_string()?;
        file.seek(SeekFrom::Start(pos)).map_err(AnonLocErr::Seek)?;
        cmd.push_str(&script);
        if !script.ends_with('\n') {
            cmd.push('\n');
        }
    }

    Ok(())
}

/// Source shell scripts with a given set of input variables and capture the resulting variables.
///
/// Only use shell-friendly variable names, as this can otherwise be used to inject arbitrary shell
/// code.
pub fn query_shell_scripts(
    script_fds: &[BorrowedFd],
    input_vars: &HashMap<&str, &str>,
    output_vars: &[&str],
    credentials: Option<&ProcessCredentials>,
) -> Result<HashMap<String, String>, AnonLocErr> {
    let (pipe_read, pipe_write) = pipe().map_err(|e| AnonLocErr::CreatePipe(e.into()))?;

    debug_assert!(
        output_vars
            .iter()
            .all(|var| var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    );

    // Construct a command which sources the scripts and writes the requested resulting variables
    // back via the inherited pipe fd.
    //
    // The resulting string looks something like:
    // ```sh
    // set -eu;
    // <contents of script 1>
    // <contents of script 2>
    // printf "VAR1=%s\0VAR2=%s\0" "${VAR1:-}" "${VAR2:-}" >&5
    // ```
    let mut cmd = String::new();
    cmd.push_str("set -eu; ");
    append_inline_shell_scripts(&mut cmd, script_fds)?;
    cmd.push_str("printf \"");
    for var in output_vars {
        cmd.push_str(var);
        cmd.push_str("=%s\\00");
    }
    cmd.push_str("\" ");
    for var in output_vars {
        cmd.push_str("\"${");
        cmd.push_str(var);
        cmd.push_str(":-}\" ");
    }
    cmd.push_str(&format!(">&{pipe_write}\0"));

    // Safety: `fork()` is `unsafe{}` because of concerns around signal handling safety.
    // We're just setting envvars and `exec`ing, which are signal safe.
    //
    // Safety: setenv() is unsafe in multithreaded contexts, but we just forked a new process
    // which only has one thread.
    let child = match unsafe { fork() }.map_err(|e| AnonLocErr::Fork(e.into()))? {
        ForkResult::Child => {
            close(pipe_read).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;

            for (var, val) in input_vars {
                unsafe {
                    std::env::set_var(var, val);
                }
            }
            if let Some(credentials) = credentials {
                drop_process_credentials(credentials)?;
            }

            let args = [
                c"sh",
                c"-c",
                CStr::from_bytes_with_nul(cmd.as_bytes()).unwrap(),
            ];
            execvp(args[0], &args).map_err(|e| AnonLocErr::ShellExec(e.into()))?;
            // Should be unreachable due to previous line's execvp returning on success and `?` on
            // failure.
            exit(1);
        }
        ForkResult::Parent { child } => child,
    };

    // Read results
    //
    // Read/writes to/from pipes are atomic within `PIPE_BUF` size, making it a good default
    // buffer size.
    //
    // Cap total output to guard against OOM from a buggy or malicious script.  The output is
    // just `VAR=value\0` pairs for package metadata, so SMALL_FILE_MAX_SIZE is far more than
    // legitimate usage requires.
    let mut buf = Vec::with_capacity(PIPE_BUF);
    buf.resize(PIPE_BUF, b'\0');
    let mut cursor = 0;
    close(pipe_write).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;
    loop {
        if buf.len() == cursor {
            if buf.len() >= SMALL_FILE_MAX_SIZE {
                close(pipe_read).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;
                return Err(AnonLocErr::ShellMsgCorrupt(
                    "output exceeded SMALL_FILE_MAX_SIZE",
                ));
            }
            buf.resize(buf.len() * 2, b'\0');
        }
        match read(pipe_read, &mut buf[cursor..]) {
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(AnonLocErr::ReadPipe(e.into())),
            Ok(0) => break,
            Ok(len) => cursor += len,
        }
    }
    close(pipe_read).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;

    // Process results into hashmap
    let mut map = HashMap::<String, String>::new();
    for var_and_val in buf.split(|&b| b == b'\0').filter(|s| !s.is_empty()) {
        let equals_index = var_and_val
            .iter()
            .position(|&b| b == b'=')
            .ok_or(AnonLocErr::ShellMsgCorrupt("Missing `=`"))?;
        let (var, val) = var_and_val.split_at(equals_index);
        let var = var
            .into_string()
            .map_err(|_| AnonLocErr::ShellMsgCorrupt("invalid utf-8"))?;
        let val = val[1..] // remove `=`
            .into_string()
            .map_err(|_| AnonLocErr::ShellMsgCorrupt("invalid utf-8"))?;
        map.insert(var, val);
    }

    match waitpid(child, None) {
        Ok(WaitStatus::Exited(_, 0)) => Ok(map),
        Ok(WaitStatus::Exited(_, rv)) => Err(AnonLocErr::ShellNonZero(rv)),
        Ok(w) => Err(AnonLocErr::ShellWaitStatus(w)),
        Err(e) => Err(AnonLocErr::ShellWait(e.into())),
    }
}

/// Run specified shell script(s).
pub fn run_shell_scripts(
    script_fds: &[BorrowedFd],
    input_vars: &HashMap<&str, &str>,
    command: &str,
    cwd: &Utf8Path,
    mut log: File,
    credentials: Option<&ProcessCredentials>,
) -> Result<(), AnonLocErr> {
    let (pipe_read, pipe_write) = pipe().map_err(|e| AnonLocErr::CreatePipe(e.into()))?;

    // Construct a command which sources the script contents and runs the specified command.
    //
    // The resulting string looks something like:
    // ```sh
    // set -eu;
    // <contents of script 1>
    // <contents of script 2>
    // cd ${cwd} && set -eux && ${command}
    // ```
    let mut cmd = String::new();
    cmd.push_str("set -eu; ");
    append_inline_shell_scripts(&mut cmd, script_fds)?;
    // Pass cwd via env var rather than interpolating into the command string to avoid
    // shell injection from paths containing characters like $, `, ", etc.
    cmd.push_str("cd \"${BPT_CWD}\" && ");
    cmd.push_str("set -eux && ");
    cmd.push_str(command);
    cmd.push('\0');

    // Safety: `fork()` is `unsafe{}` because of concerns around signal handling safety.
    // We're just setting envvars and `exec`ing, which are signal safe.
    //
    // Safety: setenv() is unsafe in multithreaded contexts, but we just forked a new process
    // which only has one thread.
    let child = match unsafe { fork() }.map_err(|e| AnonLocErr::Fork(e.into()))? {
        ForkResult::Child => {
            close(pipe_read).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;
            dup2(pipe_write, STDOUT_FILENO).map_err(AnonLocErr::Dup)?;
            dup2(pipe_write, STDERR_FILENO).map_err(AnonLocErr::Dup)?;

            for (var, val) in input_vars {
                unsafe { std::env::set_var(var, val) };
            }
            // Set corresponding env var for "cd" command above; see comment there.
            unsafe { std::env::set_var("BPT_CWD", cwd.as_str()) };
            if let Some(credentials) = credentials {
                drop_process_credentials(credentials)?;
            }

            let args = [
                c"sh",
                c"-c",
                CStr::from_bytes_with_nul(cmd.as_bytes()).unwrap(),
            ];
            execvp(args[0], &args).map_err(|e| AnonLocErr::ShellExec(e.into()))?;
            // Should be unreachable due to previous line's execvp returning on success and `?` on
            // failure.
            exit(1);
        }
        ForkResult::Parent { child } => child,
    };
    close(pipe_write).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;

    // `tee` stdout/stderr to file and stdout
    let tee = std::thread::spawn(move || -> Result<(), AnonLocErr> {
        let mut buf = [b'\0'; PIPE_BUF];
        let mut stdout = std::io::stdout().lock();
        let mut pollfds = [PollFd::new(
            pipe_read,
            PollFlags::POLLIN | PollFlags::POLLHUP,
        )];
        while poll(&mut pollfds, -1).is_ok() {
            if pollfds[0].revents().unwrap().contains(PollFlags::POLLIN) {
                if let Ok(bytes_read) = read(pipe_read, &mut buf) {
                    stdout
                        .write_all(&buf[..bytes_read])
                        .map_err(AnonLocErr::Write)?;
                    log.write_all(&buf[..bytes_read])
                        .map_err(AnonLocErr::Write)?;
                }
            }
            if pollfds[0].revents().unwrap().contains(PollFlags::POLLNVAL)
                || pollfds[0].revents().unwrap().contains(PollFlags::POLLHUP)
            {
                break;
            }
        }
        Ok(())
    });

    match waitpid(child, None) {
        Ok(WaitStatus::Exited(_, 0)) => {
            close(pipe_read).map_err(|e| AnonLocErr::ClosePipe(e.into()))?;
            tee.join().map_err(|_| AnonLocErr::UnexpectedData)??;
            Ok(())
        }
        Ok(WaitStatus::Exited(_, rv)) => Err(AnonLocErr::ShellNonZero(rv)),
        Ok(w) => Err(AnonLocErr::ShellWaitStatus(w)),
        Err(e) => Err(AnonLocErr::ShellWait(e.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{io::FileAux, testutil::unit_test_tmp_dir};
    use camino::Utf8PathBuf;
    use std::{
        fs,
        os::{fd::AsFd, unix::fs::PermissionsExt},
    };

    fn create_unreadable_script(test_name: &str, contents: &str) -> (Utf8PathBuf, File) {
        let dir = unit_test_tmp_dir("shell", test_name);
        let path = dir.join("script.sh");
        fs::write(&path, contents).unwrap();
        let file = File::open_ro(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        (dir, file)
    }

    #[test]
    fn query_shell_scripts_reads_open_script_after_path_becomes_unreadable() {
        let (_dir, file) = create_unreadable_script(
            "query_shell_scripts_reads_open_script_after_path_becomes_unreadable",
            "#!/bin/sh\npkgname=\"from-open-fd\"\n",
        );

        let vars =
            query_shell_scripts(&[file.as_fd()], &HashMap::new(), &["pkgname"], None).unwrap();

        assert_eq!(vars.get("pkgname"), Some(&"from-open-fd".to_owned()));
    }

    #[test]
    fn run_shell_scripts_executes_open_script_after_path_becomes_unreadable() {
        let (dir, file) = create_unreadable_script(
            "run_shell_scripts_executes_open_script_after_path_becomes_unreadable",
            "#!/bin/sh\nbuild() {\n\tprintf 'built-from-open-fd\\n'\n}\n",
        );
        let log_path = dir.join("build.log");
        let log = File::create(&log_path).unwrap();

        run_shell_scripts(&[file.as_fd()], &HashMap::new(), "build", &dir, log, None).unwrap();

        let log = fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("built-from-open-fd"));
    }
}
