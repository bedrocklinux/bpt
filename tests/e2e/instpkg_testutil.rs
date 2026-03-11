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

pub fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap()
}

pub fn rewrite_bpt_owner_to_current_ids(path: &str) {
    use nix::unistd::{getgid, getuid};
    use std::io::{Cursor, Read};

    const BPT_MAGIC: &[u8] = b"bpt\0";

    let uid = getuid().as_raw() as u64;
    let gid = getgid().as_raw() as u64;
    let bytes = fs::read(path).unwrap();
    assert!(
        bytes.starts_with(BPT_MAGIC),
        "expected `{path}` to begin with bpt magic"
    );

    let tarball = zstd::stream::decode_all(Cursor::new(&bytes[BPT_MAGIC.len()..])).unwrap();
    let mut archive = tar::Archive::new(Cursor::new(tarball));
    let mut rebuilt = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut rebuilt);
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let mut header = entry.header().clone();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            header.set_uid(uid);
            header.set_gid(gid);
            header.set_cksum();
            builder.append(&header, Cursor::new(data)).unwrap();
        }
        builder.finish().unwrap();
    }

    let compressed = zstd::stream::encode_all(Cursor::new(rebuilt), 0).unwrap();
    let mut out = Vec::with_capacity(BPT_MAGIC.len() + compressed.len());
    out.extend_from_slice(BPT_MAGIC);
    out.extend_from_slice(&compressed);
    fs::write(path, out).unwrap();
}
