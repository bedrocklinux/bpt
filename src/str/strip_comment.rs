/// Removes comments and trims whitespace from a string.
///
/// Does not support escaped comments.
pub trait StripComment {
    fn strip_comment(&self) -> &str;
}

impl StripComment for str {
    fn strip_comment(&self) -> &str {
        match self.split_once('#') {
            Some((precomment, _)) => precomment.trim(),
            None => self.trim(),
        }
    }
}
