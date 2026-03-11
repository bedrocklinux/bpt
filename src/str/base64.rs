use crate::{constant::*, error::*};
use base64::Engine;

pub trait Base64Encode {
    fn base64_encode(&self) -> String;
}

impl Base64Encode for [u8] {
    fn base64_encode(&self) -> String {
        BASE64_CHARSET.encode(self)
    }
}

pub trait Base64Decode {
    fn base64_decode(&self) -> Result<Vec<u8>, AnonLocErr>;
}

impl Base64Decode for [u8] {
    fn base64_decode(&self) -> Result<Vec<u8>, AnonLocErr> {
        BASE64_CHARSET
            .decode(self)
            .map_err(AnonLocErr::Base64Decode)
    }
}
