use crate::{error::*, io::BoundedFile};
use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};

/// Magic numbers are fixed-size strings used to identify file types.
///
/// Trait for magic numbers.
pub trait MagicNumber {
    /// Description of the expected file type.  Used in error messages.
    const DESCRIPTION: &'static str;
    /// The magic number itself
    const MAGIC: &'static [u8];
}

fn check_magic<M: MagicNumber>(file: &mut (impl Read + Seek)) -> Result<(), AnonLocErr> {
    const MAX_MAGIC_LEN: usize = 16;
    debug_assert!(M::MAGIC.len() <= MAX_MAGIC_LEN);

    file.seek(SeekFrom::Start(0)).map_err(AnonLocErr::Seek)?;
    let mut buf = [0u8; MAX_MAGIC_LEN];
    let invalid = AnonLocErr::InvalidMagicNumber(M::DESCRIPTION);

    match file.read_exact(&mut buf[..M::MAGIC.len()]) {
        Ok(_) if buf[..M::MAGIC.len()] == *M::MAGIC => Ok(()),
        Ok(_) => Err(invalid),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Err(invalid),
        Err(e) => Err(AnonLocErr::Read(e)),
    }
}

pub trait VerifyMagic: Read + Seek {
    fn verify_magic<M: MagicNumber>(&mut self) -> Result<(), AnonLocErr>;
    fn verify_and_strip_magic<M: MagicNumber>(self) -> Result<BoundedFile, AnonLocErr>;
}

impl VerifyMagic for BoundedFile {
    fn verify_magic<M: MagicNumber>(&mut self) -> Result<(), AnonLocErr> {
        check_magic::<M>(self)
    }

    fn verify_and_strip_magic<M: MagicNumber>(mut self) -> Result<BoundedFile, AnonLocErr> {
        self.verify_magic::<M>()?;
        self.increase_lower_bound_by(M::MAGIC.len() as u64)?;
        Ok(self)
    }
}

impl VerifyMagic for File {
    fn verify_magic<M: MagicNumber>(&mut self) -> Result<(), AnonLocErr> {
        check_magic::<M>(self)
    }

    fn verify_and_strip_magic<M: MagicNumber>(self) -> Result<BoundedFile, AnonLocErr> {
        BoundedFile::from_file(self)?.verify_and_strip_magic::<M>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::BoundedFile;

    const TMPNAME: &std::ffi::CStr = c"test-tmpfile";

    struct PngFileType;
    impl MagicNumber for PngFileType {
        const DESCRIPTION: &'static str = "PNG";
        const MAGIC: &'static [u8] = b"\x89PNG\r\n\x1a\n";
    }

    struct JpegFileType;
    impl MagicNumber for JpegFileType {
        const DESCRIPTION: &'static str = "JPEG";
        const MAGIC: &'static [u8] = b"\xFF\xD8\xFF";
    }

    #[test]
    fn test_verify_magic_png() {
        let data: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0xAA, 0xBB];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        file.verify_magic::<PngFileType>().unwrap();

        // Seek head should be positioned right after the magic number
        let mut rest = Vec::new();
        file.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, vec![0xAA, 0xBB]);
    }

    #[test]
    fn test_verify_magic_jpeg() {
        let data: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0x01, 0x02, 0x03];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        file.verify_magic::<JpegFileType>().unwrap();

        // Seek head should be positioned right after the magic number
        let mut rest = Vec::new();
        file.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_verify_magic_fail_png() {
        let data: Vec<u8> = vec![0xFF, 0xD8, 0xFF];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        let result = file.verify_magic::<PngFileType>();
        assert!(matches!(result, Err(AnonLocErr::InvalidMagicNumber("PNG"))));
    }

    #[test]
    fn test_verify_magic_fail_jpeg() {
        let data: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        let mut file = BoundedFile::create_memfd(TMPNAME, &data).unwrap();

        let result = file.verify_magic::<JpegFileType>();
        assert!(matches!(
            result,
            Err(AnonLocErr::InvalidMagicNumber("JPEG"))
        ));
    }
}
