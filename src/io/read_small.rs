//! Shared helper code for bounded small-file reads.
//!
//! Support for reading files we expect to be small such that they can be read entirely to memory
//! rather than streamed, avoiding OOMs on unexpectedly large files.

use crate::{constant::SMALL_FILE_MAX_SIZE, error::*};
use std::io::Read;

/// Return an error if the caller-provided logical length exceeds the global small-file bound.
fn ensure_small(logical_len: u64) -> Result<(), AnonLocErr> {
    if logical_len > SMALL_FILE_MAX_SIZE as u64 {
        return Err(AnonLocErr::FileTooLarge(SMALL_FILE_MAX_SIZE));
    }
    Ok(())
}

/// Read a small UTF-8 string from the current reader position.
///
/// `logical_len` is the caller's notion of remaining readable bytes (for example, from file
/// metadata or a bounded window).
pub(crate) fn read_small_file_string<R: Read + ?Sized>(
    reader: &mut R,
    logical_len: u64,
) -> Result<String, AnonLocErr> {
    ensure_small(logical_len)?;

    let mut buf = String::new();
    reader
        .take(SMALL_FILE_MAX_SIZE as u64)
        .read_to_string(&mut buf)
        .map_err(AnonLocErr::Read)?;
    Ok(buf)
}

/// Read small raw bytes from the current reader position.
///
/// `logical_len` is the caller's notion of remaining readable bytes (for example, from file
/// metadata or a bounded window).
pub(crate) fn read_small_file_bytes<R: Read + ?Sized>(
    reader: &mut R,
    logical_len: u64,
) -> Result<Vec<u8>, AnonLocErr> {
    ensure_small(logical_len)?;

    let mut buf = Vec::new();
    reader
        .take(SMALL_FILE_MAX_SIZE as u64)
        .read_to_end(&mut buf)
        .map_err(AnonLocErr::Read)?;
    Ok(buf)
}
