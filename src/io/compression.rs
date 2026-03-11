use crate::{constant::*, error::*};
use std::io::{BufReader, Chain, Cursor, ErrorKind, Read};
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;

pub enum CompressionEncoder<'a, W: std::io::Write> {
    Zstd(ZstdEncoder<'a, W>),
}

pub enum CompressionDecoder<'a, R: std::io::Read> {
    Zstd(ZstdDecoder<'a, BufReader<R>>),
}

impl<W: std::io::Write> CompressionEncoder<'_, W> {
    pub fn new(w: W) -> Result<Self, AnonLocErr> {
        ZstdEncoder::new(w, *zstd::compression_level_range().end())
            .map_err(AnonLocErr::Compress)
            .map(Self::Zstd)
    }

    pub fn finish(self) -> Result<W, AnonLocErr> {
        match self {
            CompressionEncoder::Zstd(zstd) => zstd.finish().map_err(AnonLocErr::Compress),
        }
    }
}

impl<W: std::io::Write> std::io::Write for CompressionEncoder<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            CompressionEncoder::Zstd(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            CompressionEncoder::Zstd(w) => w.flush(),
        }
    }
}

impl<R: std::io::Read> CompressionDecoder<'_, Chain<Cursor<[u8; 4]>, R>> {
    pub fn new(mut r: R) -> Result<Self, AnonLocErr> {
        let mut magic_number = [0u8; 4];
        r.read_exact(&mut magic_number).map_err(AnonLocErr::Read)?;

        // Reconstitute the full stream: chain the already-read magic bytes back with the remainder
        let r = Cursor::new(magic_number).chain(r);

        match magic_number {
            ZSTD_MAGIC | ZSTD_DICT_MAGIC => Ok(CompressionDecoder::Zstd(
                ZstdDecoder::new(r).map_err(AnonLocErr::Decompress)?,
            )),
            _ => Err(AnonLocErr::Decompress(std::io::Error::new(
                ErrorKind::InvalidData,
                "Unrecognized compression magic number",
            ))),
        }
    }
}

impl<R: std::io::Read> std::io::Read for CompressionDecoder<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            CompressionDecoder::Zstd(r) => r.read(buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::FileAux;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn test_zstd_compression_and_decompression() {
        let name = c"test_compression";
        let data = b"Hello, zstd compression!";
        let file = File::create_memfd(name, &[]).unwrap();

        let mut encoder = CompressionEncoder::new(file).unwrap();
        encoder.write_all(data).unwrap();
        let mut file = encoder.finish().unwrap();

        // Decompress the data
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut decompressed_data = vec![0u8; data.len()];
        let mut decoder = CompressionDecoder::new(file).unwrap();
        decoder.read_exact(&mut decompressed_data).unwrap();

        // Verify that the decompressed data matches the original data
        assert_eq!(data, &decompressed_data[..]);
    }

    #[test]
    fn test_invalid_magic_number() {
        let invalid_magic_number = b"ABCD";
        let name = c"test_invalid_magic_number";
        let mut file = File::create_memfd(name, &[]).unwrap();

        file.write_all(invalid_magic_number).unwrap();

        file.seek(SeekFrom::Start(0)).unwrap();
        let result = CompressionDecoder::new(file);

        match result {
            Err(AnonLocErr::Decompress(e)) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
                assert_eq!(e.to_string(), "Unrecognized compression magic number");
            }
            _ => panic!("expected decompress error"),
        };
    }

    #[test]
    fn test_compressed_data_written_in_chunks() {
        let chunk1 = b"Hello, ";
        let chunk2 = b"zstd compression!";
        let mut expected_data = Vec::new();
        expected_data.extend_from_slice(chunk1);
        expected_data.extend_from_slice(chunk2);
        let name = c"test_chunks";
        let file = File::create_memfd(name, &[]).unwrap();

        // Compress the data in chunks
        let mut encoder = CompressionEncoder::new(file).unwrap();
        encoder.write_all(chunk1).unwrap();
        encoder.write_all(chunk2).unwrap();
        let mut file = encoder.finish().unwrap();

        // Rewind the file to the beginning
        file.seek(SeekFrom::Start(0)).unwrap();

        // Decompress the data
        let mut decompressed_data = vec![0u8; expected_data.len()];
        {
            let mut decoder = CompressionDecoder::new(file).unwrap();
            decoder.read_exact(&mut decompressed_data).unwrap();
        }

        // Verify that the decompressed data matches the original data
        assert_eq!(expected_data, decompressed_data);
    }
}
