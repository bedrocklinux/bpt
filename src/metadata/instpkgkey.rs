/// InstPkg Field key.  See [crate::marshalling::Field::KEY] for more information.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum InstPkgKey {
    Dir = b'd',
    Filename = b'f',
    Gid = b'g',
    // Hardlink = b'h',
    Mode = b'm',
    RegFile = b'r',
    Subdir = b'D',
    Symlink = b's',
    Uid = b'u',
}
