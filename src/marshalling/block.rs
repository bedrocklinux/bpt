/// An iterator over double-null separated blocks.
pub struct BlockIter<'a>(&'a [u8]);

impl<'a> BlockIter<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self(buf)
    }
}

impl<'a> Iterator for BlockIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }

        let sep = self.0.windows(2).position(|window| window == [0, 0]);

        match sep {
            Some(pos) => {
                let (head, tail) = self.0.split_at(pos);
                self.0 = &tail[2..]; // Skip over the double-null.
                Some(head)
            }
            None => {
                let left = self.0;
                self.0 = &[];
                Some(left)
            }
        }
    }
}

pub trait AsBlockIter<'a> {
    fn as_block_iter(&'a self) -> BlockIter<'a>;
}

impl<'a> AsBlockIter<'a> for [u8] {
    fn as_block_iter(&'a self) -> BlockIter<'a> {
        BlockIter::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::BlockIter;

    #[test]
    fn test_empty() {
        let buf = [];
        let mut iter = BlockIter::new(&buf);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_single_empty_block() {
        let buf = [0, 0];
        let mut iter = BlockIter::new(&buf);
        assert_eq!(iter.next(), Some(&[][..]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_single_nonempty_block() {
        let buf = [1, 2, 3, 0, 0];
        let mut iter = BlockIter::new(&buf);
        assert_eq!(iter.next(), Some(&[1, 2, 3][..]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_multiple_blocks() {
        let buf = [1, 2, 3, 0, 0, 4, 5, 6, 0, 0, 7, 8, 9];
        let mut iter = BlockIter::new(&buf);
        assert_eq!(iter.next(), Some(&[1, 2, 3][..]));
        assert_eq!(iter.next(), Some(&[4, 5, 6][..]));
        assert_eq!(iter.next(), Some(&[7, 8, 9][..]));
        assert_eq!(iter.next(), None);
    }
}
