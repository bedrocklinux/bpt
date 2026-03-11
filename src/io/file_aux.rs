//! Auxiliary helper methods for std::fs::File
use crate::{error::*, io::*};
use camino::Utf8Path;
use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

pub trait FileAux {
    fn open_ro(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn open_rw(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn open_nocloexec(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn create_rw(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn create_or_open_rw(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn create_or_open_ro(path: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn create_anon(dir: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn create_memfd(name: &std::ffi::CStr, contents: &[u8]) -> Result<Self, Err>
    where
        Self: Sized;
    fn clone_anon_into(&mut self, dir: &Utf8Path) -> Result<Self, Err>
    where
        Self: Sized;
    fn link(&self, path: &Utf8Path) -> Result<(), Err>;
    fn lock_ro(&self, lock_name: &str) -> Result<(), AnonLocErr>;
    fn lock_rw(&self, lock_name: &str) -> Result<(), AnonLocErr>;
    fn copy_into_dir(&mut self, dir: &Utf8Path) -> Result<File, Err>
    where
        Self: Sized;
    /// Read the entirety of a file we expect to be small, as a String.
    ///
    /// If the file is larger than SMALL_FILE_MAX_SIZE, error.
    fn read_small_file_string(&mut self) -> Result<String, AnonLocErr>;
    /// Read the entirety of a file we expect to be small, as raw bytes.
    ///
    /// If the file is larger than SMALL_FILE_MAX_SIZE, error.
    #[cfg(test)]
    fn read_small_file_bytes(&mut self) -> Result<Vec<u8>, AnonLocErr>;
}

pub trait BorrowedFdAux {
    /// By default, Rust's `File` type sets `O_CLOEXEC`.  This means the child shell process will not
    /// be able to read the file descriptor.  If we want the child process to read it, unset the flag.
    fn unset_cloexec(&self) -> Result<(), AnonLocErr>
    where
        Self: Sized;
}

impl FileAux for File {
    fn open_ro(path: &Utf8Path) -> Result<Self, Err> {
        OpenOptions::new()
            .read(true)
            .write(false)
            .open(path)
            .map_err(AnonLocErr::Open)
            .loc(path)
    }

    fn open_rw(path: &Utf8Path) -> Result<Self, Err> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(AnonLocErr::Open)
            .loc(path)
    }

    /// Open a [File] without setting `O_CLOEXEC`
    ///
    /// Rust's default `File`/`OpenOptions` automatically sets `O_CLOEXEC`.
    /// This is undesirable if we want a child process to be able to read the file.
    fn open_nocloexec(path: &Utf8Path) -> Result<Self, Err> {
        // Use nix's open which does not set O_CLOEXEC by default
        nix::fcntl::open(
            path.as_str(),
            nix::fcntl::OFlag::O_RDONLY,
            nix::sys::stat::Mode::empty(),
        )
        // Safety: we just opened the raw fd; guaranteed to be owned by us
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .map_err(|e| AnonLocErr::Open(e.into()))
        .map(Self::from)
        .loc(path)
    }

    fn create_rw(path: &Utf8Path) -> Result<Self, Err> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(AnonLocErr::Open)
            .loc(path)
    }

    fn create_or_open_rw(path: &Utf8Path) -> Result<Self, Err> {
        let oflags =
            nix::fcntl::OFlag::O_RDWR | nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_CLOEXEC;
        let mode = nix::sys::stat::Mode::from_bits_truncate(0o644);

        nix::fcntl::open(path.as_str(), oflags, mode)
            // Safety: we just opened the raw fd; guaranteed to be owned by us
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .map_err(|e| AnonLocErr::Open(e.into()))
            .map(Self::from)
            .loc(path)
    }

    fn create_or_open_ro(path: &Utf8Path) -> Result<Self, Err> {
        let oflags =
            nix::fcntl::OFlag::O_RDONLY | nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_CLOEXEC;
        let mode = nix::sys::stat::Mode::from_bits_truncate(0o644);

        nix::fcntl::open(path.as_str(), oflags, mode)
            // Safety: we just opened the raw fd; guaranteed to be owned by us
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .map_err(|e| AnonLocErr::Open(e.into()))
            .map(Self::from)
            .loc(path)
    }

    /// Create an anonymous file within the specified directory.  Call `.link()` on the file once
    /// it is populated to link it into the filesystem.
    fn create_anon(dir: &Utf8Path) -> Result<Self, Err> {
        let mut oflags = nix::fcntl::OFlag::O_RDWR;
        oflags.insert(nix::fcntl::OFlag::O_TMPFILE);

        let mut mode = nix::sys::stat::Mode::S_IRUSR;
        mode.insert(nix::sys::stat::Mode::S_IWUSR);
        mode.insert(nix::sys::stat::Mode::S_IRGRP);
        mode.insert(nix::sys::stat::Mode::S_IROTH);

        nix::fcntl::open(dir.as_str(), oflags, mode)
            // Safety: we just opened the raw fd, guaranteed to be owned by us
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .map_err(|e| AnonLocErr::CreateAnon(e.into()))
            .map(Self::from)
            .loc(dir)
    }

    fn create_memfd(name: &std::ffi::CStr, contents: &[u8]) -> Result<Self, Err> {
        let mut file =
            nix::sys::memfd::memfd_create(name, nix::sys::memfd::MemFdCreateFlag::empty())
                .map_err(|e| Err::CreateMemFd(e.into()))
                // Safety: we just opened the raw fd, guaranteed to be owned by us
                .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
                .map(Self::from)?;

        if !contents.is_empty() {
            file.write_all(contents)
                .map_err(|e| Err::Write("<memfd>".to_string(), e))?;
            file.seek(SeekFrom::Start(0))
                .map_err(|e| Err::Seek("<memfd>".to_string(), e))?;
        }

        Ok(file)
    }

    fn clone_anon_into(&mut self, dir: &Utf8Path) -> Result<Self, Err> {
        let mut new_file = File::create_anon(dir)?;
        std::io::copy(self, &mut new_file)
            .map_err(|e| Err::Write(dir.join("<anon>").into_string(), e))?;
        Ok(new_file)
    }

    fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        // This is necessary to link file via /proc/self/fd/<fd> symlink
        let flags = nix::unistd::LinkatFlags::SymlinkFollow;

        let src = format!("/proc/self/fd/{}", self.as_fd().as_raw_fd());

        nix::unistd::linkat(None, src.as_str(), None, path.as_str(), flags)
            .map_err(|e| Err::Link(path.to_string(), e.into()))
    }

    // Advisory lock on file to coordinate across bpt instances.
    // Automatically closes on Drop or if process exits.
    fn lock_ro(&self, lock_name: &str) -> Result<(), AnonLocErr> {
        lock(self, lock_name, true)
    }

    // Advisory lock on file to coordinate across bpt instances.
    // Automatically closes on Drop or if process exits.
    fn lock_rw(&self, lock_name: &str) -> Result<(), AnonLocErr> {
        lock(self, lock_name, false)
    }

    // Doesn't rewind afterwards
    fn copy_into_dir(&mut self, dir: &Utf8Path) -> Result<File, Err> {
        self.rewind()
            .map_err(|e| Err::Seek("<copy source>".to_string(), e))?;
        let mut output = File::create_anon(dir)?;

        std::io::copy(self, &mut output)
            .map(|_| ())
            .map_err(|e| Err::Write(dir.join("<anon>").into_string(), e))?;

        Ok(output)
    }

    fn read_small_file_string(&mut self) -> Result<String, AnonLocErr> {
        let len = self.metadata().map_err(AnonLocErr::Stat)?.len();
        let pos = self.stream_position().map_err(AnonLocErr::Seek)?;
        let remaining = len.saturating_sub(pos);

        read_small_file_string(self, remaining)
    }

    #[cfg(test)]
    fn read_small_file_bytes(&mut self) -> Result<Vec<u8>, AnonLocErr> {
        let len = self.metadata().map_err(AnonLocErr::Stat)?.len();
        let pos = self.stream_position().map_err(AnonLocErr::Seek)?;
        let remaining = len.saturating_sub(pos);

        read_small_file_bytes(self, remaining)
    }
}

fn lock(file: &File, lock_name: &str, read_only: bool) -> Result<(), AnonLocErr> {
    // Try non-blocking so we can print user-facing warning before blocking
    let lock_arg = if read_only {
        nix::fcntl::FlockArg::LockSharedNonblock
    } else {
        nix::fcntl::FlockArg::LockExclusiveNonblock
    };

    match nix::fcntl::flock(file.as_raw_fd(), lock_arg) {
        Result::Err(nix::errno::Errno::EWOULDBLOCK) => {}
        Ok(()) => return Ok(()),
        Result::Err(e) => return Err(AnonLocErr::Lock(e.into())),
    }

    // Someone else has it locked.  Try again with an explanation to the user.
    use crate::color::Color;
    print!(
        "{}Another bpt instance has locked {}.  Waiting for it to finish... {}",
        Color::Warn,
        lock_name,
        Color::Default,
    );
    std::io::stdout().flush().map_err(AnonLocErr::FlushStdout)?;

    let lock_arg = if read_only {
        nix::fcntl::FlockArg::LockShared
    } else {
        nix::fcntl::FlockArg::LockExclusive
    };

    match nix::fcntl::flock(file.as_raw_fd(), lock_arg) {
        Ok(()) => {
            println!("{}done.{}", Color::Success, Color::Default);
            Ok(())
        }
        Result::Err(e) => {
            println!("failed.");
            Err(AnonLocErr::Lock(e.into()))
        }
    }
}

impl BorrowedFdAux for BorrowedFd<'_> {
    fn unset_cloexec(&self) -> Result<(), AnonLocErr> {
        let arg = nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::empty());
        nix::fcntl::fcntl(self.as_raw_fd(), arg)
            .map_err(|e| AnonLocErr::Fcntl(e.into()))
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant::SMALL_FILE_MAX_SIZE;
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("file_aux", name)
    }

    #[test]
    fn read_small_file_string_rejects_oversized_file() {
        let dir = test_dir("read_small_file_string_rejects_oversized_file");
        let path = dir.join("too-large.txt");
        std::fs::write(&path, vec![b'a'; SMALL_FILE_MAX_SIZE + 1]).unwrap();

        let mut file = File::open_ro(path.as_path()).unwrap();
        let err = file.read_small_file_string().unwrap_err();
        assert!(matches!(err, AnonLocErr::FileTooLarge(SMALL_FILE_MAX_SIZE)));
    }

    #[test]
    fn read_small_file_bytes_accepts_boundary_size() {
        let dir = test_dir("read_small_file_bytes_accepts_boundary_size");
        let path = dir.join("boundary.bin");
        let input = vec![42_u8; SMALL_FILE_MAX_SIZE];
        std::fs::write(&path, &input).unwrap();

        let mut file = File::open_ro(path.as_path()).unwrap();
        let out = file.read_small_file_bytes().unwrap();
        assert_eq!(out.len(), SMALL_FILE_MAX_SIZE);
        assert_eq!(out, input);
    }

    #[test]
    fn read_small_file_bytes_uses_remaining_len_from_current_position() {
        let dir = test_dir("read_small_file_bytes_uses_remaining_len_from_current_position");
        let path = dir.join("large.bin");
        let input = vec![9_u8; SMALL_FILE_MAX_SIZE + 5];
        std::fs::write(&path, &input).unwrap();

        let mut file = File::open_ro(path.as_path()).unwrap();
        file.seek(SeekFrom::Start(10)).unwrap();

        let out = file.read_small_file_bytes().unwrap();
        assert_eq!(out.len(), SMALL_FILE_MAX_SIZE - 5);
        assert!(out.iter().all(|b| *b == 9));
    }

    #[test]
    fn read_small_file_string_uses_remaining_len_from_current_position() {
        let dir = test_dir("read_small_file_string_uses_remaining_len_from_current_position");
        let path = dir.join("large.txt");
        let input = "a".repeat(SMALL_FILE_MAX_SIZE + 5);
        std::fs::write(&path, input.as_bytes()).unwrap();

        let mut file = File::open_ro(path.as_path()).unwrap();
        file.seek(SeekFrom::Start(10)).unwrap();

        let out = file.read_small_file_string().unwrap();
        assert_eq!(out.len(), SMALL_FILE_MAX_SIZE - 5);
        assert!(out.chars().all(|c| c == 'a'));
    }

    #[test]
    fn create_memfd_with_contents_is_rewound() {
        let mut file = File::create_memfd(c"file_aux_memfd", b"abc123").unwrap();
        let mut out = String::new();
        file.read_to_string(&mut out).unwrap();
        assert_eq!(out, "abc123");
    }

    #[test]
    fn open_nocloexec_clears_fd_cloexec_flag() {
        let dir = test_dir("open_nocloexec_clears_fd_cloexec_flag");
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello").unwrap();

        let file = File::open_nocloexec(path.as_path()).unwrap();
        let flags = nix::fcntl::fcntl(file.as_raw_fd(), nix::fcntl::FcntlArg::F_GETFD).unwrap();
        assert_eq!(flags & nix::libc::FD_CLOEXEC, 0);
    }

    #[test]
    fn create_or_open_rw_creates_then_opens_existing_file() {
        let dir = test_dir("create_or_open_rw_creates_then_opens_existing_file");
        let path = dir.join("rw.txt");

        {
            let mut first = File::create_or_open_rw(path.as_path()).unwrap();
            first.write_all(b"hello").unwrap();
        }

        let mut second = File::create_or_open_rw(path.as_path()).unwrap();
        let mut out = String::new();
        second.read_to_string(&mut out).unwrap();
        assert_eq!(out, "hello");
    }
}
