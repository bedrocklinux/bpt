use crate::error::*;
use std::borrow::Cow;

/// Simple encoding scheme to avoid various issues with filenames:
///
/// - We'd like to store filepaths and URLs in filenames, but `/` is disallowed in filenames
/// - `:`, `=`, and `%` confuse Make.  (This disallows percent encoding.)
/// - `'`, `"`, `\\`, `$`, `{`, and `}` confuse Bourne shell.
///
/// Map any unsafe character to `_XX` where `XX` is the hex encoding of each UTF-8 byte.
/// Multi-byte characters produce multiple `_XX` pairs (e.g., `日` → `_e6_97_a5`).
pub trait UnderscoreEncode {
    fn underscore_encode(&self) -> Cow<'_, str>;
}

impl UnderscoreEncode for str {
    fn underscore_encode(&self) -> Cow<'_, str> {
        if self.chars().all(is_underscore_safe) {
            Cow::Borrowed(self)
        } else {
            let extra: usize = self
                .chars()
                .filter(|&c| !is_underscore_safe(c))
                .map(|c| c.len_utf8() * 3 - 1) // each byte becomes _XX (3 chars), minus the original byte
                .sum();
            let mut s = String::with_capacity(self.len() + extra);
            for c in self.chars() {
                if is_underscore_safe(c) {
                    s.push(c);
                } else {
                    s.push_str(&encode_char(c));
                }
            }
            Cow::Owned(s)
        }
    }
}

impl UnderscoreEncode for String {
    fn underscore_encode(&self) -> Cow<'_, str> {
        self.as_str().underscore_encode()
    }
}

pub trait UnderscoreDecode {
    fn underscore_decode(&self) -> Result<Cow<'_, str>, Err>;
}

impl UnderscoreDecode for str {
    fn underscore_decode(&self) -> Result<Cow<'_, str>, Err> {
        if self.chars().all(|c| c != '_') {
            Ok(Cow::Borrowed(self))
        } else {
            let mut s = String::with_capacity(self.len());
            let mut bytes = Vec::new();

            let mut chars = self.chars();
            while let Some(c) = chars.next() {
                if c == '_' {
                    let byte = match (chars.next(), chars.next()) {
                        (Some(c1), Some(c2)) => decode_hex_byte(c1, c2)
                            .ok_or_else(|| Err::InvalidUnderscoreEncoding(self.to_string()))?,
                        _ => return Err(Err::InvalidUnderscoreEncoding(self.to_string())),
                    };
                    bytes.push(byte);
                } else {
                    if !bytes.is_empty() {
                        let decoded = std::str::from_utf8(&bytes)
                            .map_err(|_| Err::InvalidUnderscoreEncoding(self.to_string()))?;
                        s.push_str(decoded);
                        bytes.clear();
                    }
                    s.push(c);
                }
            }
            if !bytes.is_empty() {
                let decoded = std::str::from_utf8(&bytes)
                    .map_err(|_| Err::InvalidUnderscoreEncoding(self.to_string()))?;
                s.push_str(decoded);
            }
            Ok(Cow::Owned(s))
        }
    }
}

fn is_underscore_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

fn encode_char(c: char) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 4];
    let bytes = c.encode_utf8(&mut buf).as_bytes();
    let mut s = String::with_capacity(bytes.len() * 3);
    for &b in bytes {
        s.push('_');
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn decode_hex_byte(c1: char, c2: char) -> Option<u8> {
    let hi = c1.to_digit(16)? as u8;
    let lo = c2.to_digit(16)? as u8;
    Some(hi << 4 | lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_underscore_encode_no_special_chars() {
        let input = "SafeFileName123";
        let expected = Cow::Borrowed(input);
        let encoded = input.underscore_encode();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn test_underscore_encode_with_special_chars() {
        let input = "This is a test: with special/chars*%";
        let expected: Cow<str> =
            Cow::Owned("This_20is_20a_20test_3a_20with_20special_2fchars_2a_25".to_string());
        let encoded = input.underscore_encode();
        assert_eq!(encoded, expected);
    }

    #[test]
    fn test_underscore_decode_no_encoded_chars() {
        let input = "SafeFileName123";
        let expected = Cow::Borrowed(input);
        let decoded = input.underscore_decode().unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_underscore_decode_with_encoded_chars() {
        let input = "This_20is_20a_20test_3a_20with_20special_2fchars_2a_25";
        let expected: Cow<str> = Cow::Owned("This is a test: with special/chars*%".to_string());
        let decoded = input.underscore_decode().unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_non_ascii_roundtrip() {
        let input = "path/with/日本語";
        let encoded = input.underscore_encode();
        let decoded = encoded.underscore_decode().unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_decode_hex_byte() {
        assert_eq!(decode_hex_byte('2', '0'), Some(0x20));
        assert_eq!(decode_hex_byte('f', 'f'), Some(0xff));
        assert_eq!(decode_hex_byte('0', '0'), Some(0x00));
        assert_eq!(decode_hex_byte('3', 'a'), Some(0x3a));
        assert_eq!(decode_hex_byte('g', '0'), None);
        assert_eq!(decode_hex_byte('0', 'z'), None);
    }

    #[test]
    fn test_underscore_decode_invalid() {
        // Trailing underscore with insufficient hex digits
        assert!("foo_2".underscore_decode().is_err());
        // Invalid hex characters
        assert!("foo_zz".underscore_decode().is_err());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let inputs = vec![
            "SafeFileName123",
            "This is a test: with special/chars*%",
            "Another example_3A with_20_underscores",
        ];

        for input in inputs {
            let encoded = input.underscore_encode();
            let encoded = encoded.into_owned();
            let encoded = encoded.as_str();
            let decoded = encoded.underscore_decode();
            assert_eq!(decoded.unwrap(), input);
        }
    }

    #[test]
    fn test_encode_ascii_punctuation_matches_policy() {
        // Alnum, '.' and '-' are safe; everything else is encoded.
        for b in 0u8..=127 {
            let c = b as char;
            let input = c.to_string();
            let encoded = input.underscore_encode().into_owned();
            let is_safe = c.is_ascii_alphanumeric() || c == '.' || c == '-';
            if is_safe {
                assert_eq!(encoded, input, "expected safe char to be unchanged: {c:?}");
            } else {
                assert_eq!(
                    encoded,
                    format!("_{b:02x}"),
                    "expected encoded char for {c:?}"
                );
            }
        }
    }

    #[test]
    fn test_encode_char_utf8_multibyte_boundaries() {
        // 2-byte UTF-8
        assert_eq!("é".underscore_encode().as_ref(), "_c3_a9");
        // 3-byte UTF-8
        assert_eq!("日".underscore_encode().as_ref(), "_e6_97_a5");
        // 4-byte UTF-8
        assert_eq!("😀".underscore_encode().as_ref(), "_f0_9f_98_80");
    }

    #[test]
    fn test_decode_mixed_literal_and_encoded_fragments() {
        let encoded = "foo_2fbar-baz_20qux._5f";
        let decoded = encoded.underscore_decode().unwrap();
        assert_eq!(decoded.as_ref(), "foo/bar-baz qux._");
    }

    #[test]
    fn test_decode_uppercase_hex_is_accepted() {
        let encoded = "_2F_41_7a";
        let decoded = encoded.underscore_decode().unwrap();
        assert_eq!(decoded.as_ref(), "/Az");
    }
}
