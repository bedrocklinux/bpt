use crate::{error::*, marshalling::*};
use std::borrow::Cow;

/// Field list.
///
/// Field whose binary representation is a whitespace separated list.
pub trait FieldList {
    /// This is passed directly to [Field::NAME].
    const LIST_NAME: &'static str;
    /// This is passed directly to [Field::KEY].
    const LIST_KEY: u8;

    type Entry: Field;

    fn from_entries(entries: Vec<Self::Entry>) -> Self;

    fn as_slice(&self) -> &[Self::Entry];

    fn iter(&self) -> std::slice::Iter<'_, Self::Entry> {
        self.as_slice().iter()
    }

    fn new() -> Self
    where
        Self: Sized,
    {
        Self::from_entries(Vec::new())
    }
}

/// The field type name, name constant, and Key byte enum variant are all the same.  Use a macro to
/// avoid the possibility of an error when repeating them.
#[macro_export]
macro_rules! make_field_list {
    ($type:ident, $key_enum:ty, $entry:ident) => {
        impl $crate::marshalling::FieldList for $type {
            const LIST_NAME: &'static str = stringify!($type);
            const LIST_KEY: u8 = <$key_enum>::$type as u8;
            type Entry = $entry;
            fn as_slice(&self) -> &[Self::Entry] {
                &self.0
            }
            fn from_entries(entries: Vec<Self::Entry>) -> Self {
                Self(entries)
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for entry in &self.0 {
                    if first {
                        first = false;
                    } else {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", entry)?;
                }
                Ok(())
            }
        }

        $crate::make_display_color!($type, |s, f| {
            let mut first = true;
            for entry in &s.0 {
                if first {
                    first = false;
                } else {
                    write!(f, " ")?;
                }
                write!(f, "{}", entry.color())?;
            }
            Ok(())
        });
    };
}

impl<T: FieldList> Field for T {
    const NAME: &'static str = Self::LIST_NAME;
    const KEY: u8 = Self::LIST_KEY;
}

impl<T: FieldList> AsBytes for T {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        match self.as_slice() {
            [] => Cow::Borrowed(b""),
            [single] => single.as_bytes(),
            [first, rest @ ..] => {
                let mut bytes = Vec::from(first.as_bytes().as_ref());
                for entry in rest {
                    bytes.push(b' ');
                    bytes.extend_from_slice(entry.as_bytes().as_ref());
                }
                Cow::Owned(bytes)
            }
        }
    }
}

impl<T: FieldList> FromFieldStr for T {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        value
            .split_whitespace()
            .map(T::Entry::from_field_str)
            .collect::<Result<_, _>>()
            .map(T::from_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal test entry type
    #[derive(Debug, Clone, PartialEq)]
    struct TestEntry(FieldStr);

    impl FromFieldStr for TestEntry {
        fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
            Ok(TestEntry(value))
        }
    }

    impl AsBytes for TestEntry {
        fn as_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Borrowed(self.0.as_bytes())
        }
    }

    impl std::fmt::Display for TestEntry {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl Field for TestEntry {
        const NAME: &'static str = "TestEntry";
        const KEY: u8 = b'T';
    }

    // Minimal test list type
    struct TestList(Vec<TestEntry>);

    impl FieldList for TestList {
        const LIST_NAME: &'static str = "TestList";
        const LIST_KEY: u8 = b'L';
        type Entry = TestEntry;
        fn as_slice(&self) -> &[Self::Entry] {
            &self.0
        }
        fn from_entries(entries: Vec<Self::Entry>) -> Self {
            Self(entries)
        }
    }

    fn entry(s: &str) -> TestEntry {
        TestEntry(FieldStr::try_from(s).unwrap())
    }

    #[test]
    fn test_as_bytes_empty() {
        let list = TestList(vec![]);
        assert_eq!(AsBytes::as_bytes(&list).as_ref(), b"");
    }

    #[test]
    fn test_as_bytes_single() {
        let list = TestList(vec![entry("hello")]);
        assert_eq!(AsBytes::as_bytes(&list).as_ref(), b"hello");
    }

    #[test]
    fn test_as_bytes_multiple() {
        let list = TestList(vec![entry("a"), entry("b"), entry("c")]);
        assert_eq!(AsBytes::as_bytes(&list).as_ref(), b"a b c");
    }

    #[test]
    fn test_from_field_str_roundtrip() {
        let list = TestList(vec![entry("foo"), entry("bar"), entry("baz")]);
        let bytes = AsBytes::as_bytes(&list);
        let field_str = FieldStr::try_from(std::str::from_utf8(&bytes).unwrap()).unwrap();
        let parsed = TestList::from_field_str(field_str).unwrap();
        assert_eq!(parsed.as_slice(), list.as_slice());
    }
}
