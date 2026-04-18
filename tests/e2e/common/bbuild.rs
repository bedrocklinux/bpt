use std::{fs, path::Path};

pub fn write_modified_bbuild(src: &str, dst: &str, replacements: &[(&str, &str)]) {
    let mut contents = fs::read_to_string(src).unwrap();
    for (from, to) in replacements {
        assert!(
            contents.contains(from),
            "expected bbuild fixture to contain `{from}`"
        );
        contents = contents.replacen(from, to, 1);
    }
    if let Some(parent) = Path::new(dst).parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(dst, contents).unwrap();
}
