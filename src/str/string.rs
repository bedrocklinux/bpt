use std::io::{Error, ErrorKind};

pub trait IntoString {
    fn into_string(self) -> Result<String, std::io::Error>;
}

impl IntoString for &[u8] {
    fn into_string(self) -> Result<String, std::io::Error> {
        String::from_utf8(self.into()).map_err(|_| {
            let s = String::from_utf8_lossy(self);
            Error::new(ErrorKind::InvalidData, format!("`{s}` is not valid UTF-8"))
        })
    }
}
