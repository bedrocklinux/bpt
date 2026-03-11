pub trait Exists {
    fn exists(&self) -> bool;
}

impl Exists for &str {
    fn exists(&self) -> bool {
        std::path::Path::new(self).exists()
    }
}

pub trait Contains {
    fn contains(&self, s: &str) -> bool;
}

impl Contains for &str {
    fn contains(&self, expect: &str) -> bool {
        let contents =
            std::fs::read_to_string(self).unwrap_or_else(|e| panic!("cannot read `{self}`: {e}"));
        contents.contains(expect)
    }
}
