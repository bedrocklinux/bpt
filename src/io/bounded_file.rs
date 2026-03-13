use crate::{error::*, io::*};
#[cfg(test)]
use std::os::fd::OwnedFd;
use std::{
    cmp::min,
    fs::File,
    io::{Error, ErrorKind, Read, Seek, SeekFrom},
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
};

/// A wrapper around std::fs::File that constrains the region of the underlying Read+Seek which is
/// accessible.  This is conceptually comparable to a `Slice`, but applied to a file instead of a
/// buffer.
pub struct BoundedFile {
    inner: File,
    lower: u64,
    upper: u64,
    pos: u64,
}

impl BoundedFile {
    fn checked_signed_add(base: u64, offset: i64) -> Option<u64> {
        if offset >= 0 {
            base.checked_add(offset as u64)
        } else {
            base.checked_sub(offset.unsigned_abs())
        }
    }

    pub fn from_file(mut inner: File) -> Result<Self, AnonLocErr> {
        // Explicitly seek to beginning of the file.  Treat a new File like a new, freshly-opened
        // file with the seek head at the start of the file.
        Ok(Self {
            lower: 0,
            upper: inner.seek(SeekFrom::End(0)).map_err(AnonLocErr::Seek)?,
            pos: inner.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?,
            inner,
        })
    }

    #[cfg(test)]
    pub fn from_fd(fd: OwnedFd) -> Result<Self, AnonLocErr> {
        Self::from_file(File::from(fd))
    }

    #[cfg(test)]
    pub fn create_memfd(name: &std::ffi::CStr, contents: &[u8]) -> Result<Self, Err> {
        File::create_memfd(name, contents)
            .and_then(|f| BoundedFile::from_file(f).loc("<memfd>".to_string()))
    }

    pub fn inner(&self) -> &File {
        &self.inner
    }

    pub fn into_inner(self) -> File {
        self.inner
    }

    pub fn increase_lower_bound_by(&mut self, offset: u64) -> Result<(), AnonLocErr> {
        let new_lower = self.lower.checked_add(offset).ok_or_else(|| {
            AnonLocErr::Seek(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ))
        })?;

        if new_lower > self.upper {
            return Err(AnonLocErr::Seek(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            )));
        }

        self.lower = new_lower;
        if self.pos < self.lower {
            self.pos = self
                .inner
                .seek(SeekFrom::Start(self.lower))
                .map_err(AnonLocErr::Seek)?;
        }
        Ok(())
    }

    pub fn decrease_upper_bound_by(&mut self, offset: u64) -> Result<(), AnonLocErr> {
        let new_upper = self.upper.checked_sub(offset).ok_or_else(|| {
            AnonLocErr::Seek(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ))
        })?;

        if new_upper < self.lower {
            return Err(AnonLocErr::Seek(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            )));
        }

        self.upper = new_upper;
        if self.pos > new_upper {
            self.pos = self
                .inner
                .seek(SeekFrom::Start(self.upper))
                .map_err(AnonLocErr::Seek)?;
        }
        Ok(())
    }

    pub fn increase_upper_bound_by(&mut self, offset: u64) -> Result<(), AnonLocErr> {
        let new_upper = self.upper.checked_add(offset).ok_or_else(|| {
            AnonLocErr::Seek(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ))
        })?;

        // Given incoming invariant
        //
        // ```
        // self.lower <= self.upper
        // ```
        //
        // The `self.upper.checked_add()` cannot change the invariant and thus checking for the
        // invariant breaking is unneeded.
        //
        // if new_upper < self.lower {
        //     return Err(AnonLocErr::Seek(Error::new(
        //         ErrorKind::InvalidInput,
        //         "Seek position is out of bounds",
        //     )));
        // }

        self.upper = new_upper;
        Ok(())
    }

    #[cfg(test)]
    pub fn read_small_file_string(&mut self) -> Result<String, AnonLocErr> {
        let remaining = self.upper.saturating_sub(self.pos);
        read_small_file_string(self, remaining)
    }

    pub fn read_small_file_bytes(&mut self) -> Result<Vec<u8>, AnonLocErr> {
        let remaining = self.upper.saturating_sub(self.pos);
        read_small_file_bytes(self, remaining)
    }
}

impl Read for BoundedFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let remaining = self.upper.saturating_sub(self.pos);
        if remaining == 0 {
            return Ok(0);
        }

        let to_read = min(buf.len() as u64, remaining);
        let len = self.inner.read(&mut buf[..to_read as usize])?;
        self.pos += len as u64;
        Ok(len)
    }
}

impl Seek for BoundedFile {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, std::io::Error> {
        let overflow = Error::new(ErrorKind::InvalidInput, "Seek position overflow");

        let new_pos = match pos {
            SeekFrom::Start(n) => self
                .lower
                .checked_add(n)
                .ok_or_else(|| Error::new(overflow.kind(), overflow.to_string()))?,
            SeekFrom::End(n) => Self::checked_signed_add(self.upper, n)
                .ok_or_else(|| Error::new(overflow.kind(), overflow.to_string()))?,
            SeekFrom::Current(n) => Self::checked_signed_add(self.pos, n)
                .ok_or_else(|| Error::new(overflow.kind(), overflow.to_string()))?,
        };

        if new_pos < self.lower || new_pos > self.upper {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Seek position is out of bounds",
            ));
        }

        self.pos = self.inner.seek(SeekFrom::Start(new_pos))?;
        Ok(self.pos - self.lower)
    }
}

impl AsFd for BoundedFile {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl AsRawFd for BoundedFile {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant::SMALL_FILE_MAX_SIZE;
    use crate::io::file_aux::FileAux;

    const TMPNAME: &std::ffi::CStr = c"test-tmpfile";

    #[test]
    fn test_read() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let mut buf = [0u8; 5];
        assert_eq!(file.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"fghij");

        let mut buf = [0u8; 5];
        assert_eq!(file.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"klmno");

        let mut buf = [0u8; 5];
        assert_eq!(file.read(&mut buf).unwrap(), 0);
        assert_eq!(&buf, &[0u8; 5]);
    }

    #[test]
    fn test_read_past_upper_bound() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let mut buf = [0u8; 12];
        assert_eq!(file.read(&mut buf).unwrap(), 10);
        assert_eq!(&buf[..10], b"fghijklmno");
        assert_eq!(&buf[10..], &[0u8; 2]);
    }

    #[test]
    fn test_read_no_bounds() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();

        let mut buf = [0u8; 30];
        assert_eq!(file.read(&mut buf).unwrap(), 26);
        assert_eq!(&buf[..26], data);
        assert_eq!(&buf[26..], &[0u8; 4]);
    }

    #[test]
    fn test_read_lower_beyond_end() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();

        let result = file.increase_lower_bound_by(30);
        assert!(matches!(
            result,
            Err(AnonLocErr::Seek(e)) if e.kind() == ErrorKind::InvalidInput
        ));
    }

    #[test]
    fn test_seek_start() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let new_pos = file.seek(SeekFrom::Start(3)).unwrap();
        assert_eq!(new_pos, 3);

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"ijk");
    }

    #[test]
    fn test_seek_end() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        //  data = b"12345|fghijk|lmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let new_pos = file.seek(SeekFrom::End(-3)).unwrap();
        assert_eq!(new_pos, 7);

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"mno");
    }

    #[test]
    fn test_seek_current() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"fgh");

        let new_pos = file.seek(SeekFrom::Current(-1)).unwrap();
        assert_eq!(new_pos, 2);

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"hij");
    }

    #[test]
    fn test_seek_out_of_bounds() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let result = file.seek(SeekFrom::Start(11));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);

        let result = file.seek(SeekFrom::End(1));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);

        let result = file.seek(SeekFrom::Current(-6));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_increase_lower_bound_by() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        file.increase_lower_bound_by(3).unwrap();

        let expected_lower = 5 + 3;
        let expected_upper = 15;
        assert_eq!(file.lower, expected_lower);
        assert_eq!(file.upper, expected_upper);
        assert_eq!(file.pos, expected_lower);

        let mut buf = [0u8; 10];
        assert_eq!(file.read(&mut buf).unwrap(), 15 - 5 - 3);
        assert_eq!(&buf[..7], b"ijklmno");
        assert_eq!(&buf[7..], &[0u8; 3]);
    }

    #[test]
    fn test_decrease_upper_bound_by_clamps_pos() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        // bounds: [5, 15), pos=5

        // Advance pos past where new upper will be
        file.seek(SeekFrom::Start(8)).unwrap();
        // pos is now 5+8=13

        file.decrease_upper_bound_by(3).unwrap();
        // upper: 15-3=12, pos was 13 > 12, so clamped to 12

        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 12);
        assert_eq!(file.pos, 12);

        // At upper bound, nothing to read
        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 0);

        // Seek back and verify we can still read correctly
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = [0u8; 10];
        assert_eq!(file.read(&mut buf).unwrap(), 7);
        assert_eq!(&buf[..7], b"fghijkl");
    }

    #[test]
    fn test_decrease_upper_bound_by_preserves_valid_pos() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        // bounds: [5, 15), pos=5

        // Advance to relative position 3 (absolute 8)
        file.seek(SeekFrom::Start(3)).unwrap();
        assert_eq!(file.pos, 8);

        // Decrease upper to 12; pos (8) < 12, no clamp needed
        file.decrease_upper_bound_by(3).unwrap();
        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 12);
        assert_eq!(file.pos, 8);

        // Read should continue from current position
        let mut buf = [0u8; 10];
        assert_eq!(file.read(&mut buf).unwrap(), 4);
        assert_eq!(&buf[..4], b"ijkl");
    }

    #[test]
    fn test_decrease_upper_bound_by_underflow_errors() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();

        let result = file.decrease_upper_bound_by(30);
        assert!(matches!(
            result,
            Err(AnonLocErr::Seek(e)) if e.kind() == ErrorKind::InvalidInput
        ));
        assert_eq!(file.lower, 0);
        assert_eq!(file.upper, data.len() as u64);
        assert_eq!(file.pos, 0);
    }

    #[test]
    fn test_checked_signed_add_large_base() {
        let base = i64::MAX as u64 + 90;
        assert_eq!(BoundedFile::checked_signed_add(base, -10), Some(base - 10));
        assert_eq!(BoundedFile::checked_signed_add(base, 10), Some(base + 10));
        assert_eq!(BoundedFile::checked_signed_add(4, -5), None);
        assert_eq!(BoundedFile::checked_signed_add(u64::MAX - 1, 2), None);
        assert_eq!(
            BoundedFile::checked_signed_add(i64::MAX as u64 + 1, i64::MIN),
            Some(0)
        );
        assert_eq!(
            BoundedFile::checked_signed_add(u64::MAX, i64::MIN),
            Some(u64::MAX - (1u64 << 63))
        );
    }

    #[test]
    fn test_increase_upper_bound_by_success() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        file.increase_upper_bound_by(3).unwrap();
        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 18);
        assert_eq!(file.pos, 5);

        let mut buf = [0u8; 16];
        assert_eq!(file.read(&mut buf).unwrap(), 13);
        assert_eq!(&buf[..13], b"fghijklmnopqr");
    }

    #[test]
    fn test_increase_upper_bound_by_overflow_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = 1;
        file.upper = u64::MAX - 1;
        file.pos = 1;

        let result = file.increase_upper_bound_by(2);
        assert!(matches!(
            result,
            Err(AnonLocErr::Seek(e)) if e.kind() == ErrorKind::InvalidInput
        ));
        assert_eq!(file.lower, 1);
        assert_eq!(file.upper, u64::MAX - 1);
        assert_eq!(file.pos, 1);
    }

    #[test]
    fn test_decrease_upper_bound_by_crosses_lower_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = 10;
        file.upper = 20;
        file.pos = 10;

        let result = file.decrease_upper_bound_by(11);
        assert!(matches!(
            result,
            Err(AnonLocErr::Seek(e)) if e.kind() == ErrorKind::InvalidInput
        ));
        assert_eq!(file.lower, 10);
        assert_eq!(file.upper, 20);
        assert_eq!(file.pos, 10);
    }

    #[test]
    fn test_increase_lower_bound_by_overflow_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = u64::MAX;
        file.upper = u64::MAX;
        file.pos = u64::MAX;

        let result = file.increase_lower_bound_by(1);
        assert!(matches!(
            result,
            Err(AnonLocErr::Seek(e)) if e.kind() == ErrorKind::InvalidInput
        ));
        assert_eq!(file.lower, u64::MAX);
        assert_eq!(file.upper, u64::MAX);
        assert_eq!(file.pos, u64::MAX);
    }

    #[test]
    fn test_seek_start_at_upper_bound_is_allowed() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let pos = file.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(pos, 10);

        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_seek_current_forward_out_of_bounds_errors() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let result = file.seek(SeekFrom::Current(11));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_seek_start_overflow_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = u64::MAX - 1;
        file.upper = u64::MAX;
        file.pos = u64::MAX - 1;

        let result = file.seek(SeekFrom::Start(2));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_zero_width_range_read_and_seek() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(21).unwrap();
        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 5);

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 0);

        assert_eq!(file.seek(SeekFrom::Start(0)).unwrap(), 0);
        assert_eq!(file.seek(SeekFrom::End(0)).unwrap(), 0);
        assert_eq!(file.stream_position().unwrap(), 0);

        let result = file.seek(SeekFrom::Start(1));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_seek_end_positive_overflow_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = 0;
        file.upper = u64::MAX - 1;
        file.pos = 0;

        let result = file.seek(SeekFrom::End(2));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_seek_current_positive_overflow_errors() {
        let mut file = BoundedFile::create_memfd(TMPNAME, b"abc").unwrap();
        file.lower = 0;
        file.upper = u64::MAX;
        file.pos = u64::MAX - 1;

        let result = file.seek(SeekFrom::Current(2));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn test_read_empty_buffer_does_not_move_position() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        file.seek(SeekFrom::Start(2)).unwrap();

        let pos_before = file.pos;
        assert_eq!(file.read(&mut []).unwrap(), 0);
        assert_eq!(file.pos, pos_before);
    }

    #[test]
    fn test_from_file_resets_seek_to_start() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut inner = File::create_memfd(TMPNAME, data).unwrap();
        inner.seek(SeekFrom::Start(13)).unwrap();

        let mut file = BoundedFile::from_file(inner).unwrap();
        assert_eq!(file.pos, 0);
        assert_eq!(file.lower, 0);
        assert_eq!(file.upper, data.len() as u64);

        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"abc");
    }

    #[test]
    fn test_increase_lower_bound_by_preserves_valid_pos() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        // bounds: [0, 26), pos=0

        // Advance pos to absolute 10
        file.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(file.pos, 10);

        // Increase lower to 5; pos (10) >= 5, no clamp needed
        file.increase_lower_bound_by(5).unwrap();
        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 26);
        assert_eq!(file.pos, 10);

        // Read should continue from current position
        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"klm");
    }

    #[test]
    fn test_decrease_upper_bound_by_pos_at_new_upper() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        // bounds: [5, 15), pos=5

        // Seek to relative 7 (absolute 12)
        file.seek(SeekFrom::Start(7)).unwrap();
        assert_eq!(file.pos, 12);

        // Decrease upper to 12; pos (12) == new upper, no clamp needed
        file.decrease_upper_bound_by(3).unwrap();
        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 12);
        assert_eq!(file.pos, 12);

        // At upper bound, nothing to read
        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_increase_lower_bound_to_equal_upper() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.decrease_upper_bound_by(16).unwrap();
        // bounds: [0, 10)

        file.increase_lower_bound_by(10).unwrap();
        assert_eq!(file.lower, 10);
        assert_eq!(file.upper, 10);

        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_decrease_upper_bound_to_equal_lower() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(10).unwrap();
        // bounds: [10, 26)

        file.decrease_upper_bound_by(16).unwrap();
        assert_eq!(file.lower, 10);
        assert_eq!(file.upper, 10);
        assert_eq!(file.pos, 10);

        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn test_seek_error_preserves_pos() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        // bounds: [5, 15), pos=5

        file.seek(SeekFrom::Start(4)).unwrap();
        assert_eq!(file.pos, 9);

        // Out-of-bounds seek should fail and not change pos
        assert!(file.seek(SeekFrom::Start(11)).is_err());
        assert_eq!(file.pos, 9);

        assert!(file.seek(SeekFrom::End(1)).is_err());
        assert_eq!(file.pos, 9);

        assert!(file.seek(SeekFrom::Current(-5)).is_err());
        assert_eq!(file.pos, 9);

        // Verify reads still work from preserved position
        let mut buf = [0u8; 3];
        assert_eq!(file.read(&mut buf).unwrap(), 3);
        assert_eq!(&buf, b"jkl");
    }

    #[test]
    fn test_read_single_byte_range() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(20).unwrap();
        // bounds: [5, 6), single byte 'f'

        assert_eq!(file.lower, 5);
        assert_eq!(file.upper, 6);

        let mut buf = [0u8; 5];
        assert_eq!(file.read(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], b'f');

        // Second read should return 0
        assert_eq!(file.read(&mut buf).unwrap(), 0);

        // Seek back and re-read
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = [0u8; 1];
        assert_eq!(file.read(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], b'f');
    }

    #[test]
    fn test_sequential_bound_adjustments_with_reads() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();

        // Progressively narrow the window from both sides
        file.increase_lower_bound_by(3).unwrap();
        file.decrease_upper_bound_by(3).unwrap();
        // bounds: [3, 23)

        let mut buf = [0u8; 3];
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"def");

        file.increase_lower_bound_by(3).unwrap();
        // bounds: [6, 23), pos was 6 == new lower, no clamp
        assert_eq!(file.pos, 6);

        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ghi");

        file.decrease_upper_bound_by(5).unwrap();
        // bounds: [6, 18), pos was 9 < 18, no clamp
        assert_eq!(file.pos, 9);

        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"jkl");

        // Now expand upper back
        file.increase_upper_bound_by(3).unwrap();
        // bounds: [6, 21), pos unchanged
        assert_eq!(file.pos, 12);

        // Read remaining from current pos to new upper
        let mut buf = [0u8; 20];
        assert_eq!(file.read(&mut buf).unwrap(), 9);
        assert_eq!(&buf[..9], b"mnopqrstu");
    }

    #[test]
    fn test_from_fd() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let inner = File::create_memfd(TMPNAME, data).unwrap();
        let fd: OwnedFd = inner.into();

        let mut file = BoundedFile::from_fd(fd).unwrap();
        assert_eq!(file.pos, 0);
        assert_eq!(file.lower, 0);
        assert_eq!(file.upper, data.len() as u64);

        let mut buf = [0u8; 5];
        assert_eq!(file.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"abcde");
    }

    #[test]
    fn test_read_small_file_bytes_respects_bounded_window() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();

        let out = file.read_small_file_bytes().unwrap();
        assert_eq!(out, b"fghijklmno");
    }

    #[test]
    fn test_read_small_file_string_respects_current_position() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let mut file = BoundedFile::create_memfd(TMPNAME, data).unwrap();
        file.increase_lower_bound_by(5).unwrap();
        file.decrease_upper_bound_by(11).unwrap();
        file.seek(SeekFrom::Start(2)).unwrap();

        let out = file.read_small_file_string().unwrap();
        assert_eq!(out, "hijklmno");
    }

    #[test]
    fn test_read_small_file_bytes_uses_bounded_remaining_len_for_size_check() {
        let data = vec![b'x'; SMALL_FILE_MAX_SIZE + 5];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        file.increase_lower_bound_by(SMALL_FILE_MAX_SIZE as u64)
            .unwrap();

        let out = file.read_small_file_bytes().unwrap();
        assert_eq!(out.len(), 5);
        assert!(out.iter().all(|b| *b == b'x'));
    }

    #[test]
    fn test_read_small_file_bytes_rejects_oversized_bounded_window() {
        let data = vec![b'x'; SMALL_FILE_MAX_SIZE + 1];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        let err = file.read_small_file_bytes().unwrap_err();
        assert!(matches!(err, AnonLocErr::FileTooLarge(SMALL_FILE_MAX_SIZE)));
    }
}
