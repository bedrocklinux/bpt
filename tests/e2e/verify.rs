use crate::*;
use ::function_name::named;
use std::io::{Seek, Write};

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn verify_one() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();

    let _ = run_bpt_sign!(per_test_path!("fakeblock.bbuild")).unwrap();

    let stdout = run_bpt_verify!(per_test_path!("fakeblock.bbuild")).unwrap();
    assert!(stdout.contains("Verified"));
    assert!(stdout.contains("fakeblock.bbuild"));
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn verify_multiple() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-songs.bbuild"),
        per_test_path!("fakeblock-songs.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-song-gen.bbuild"),
        per_test_path!("fakeblock-song-gen.bbuild"),
    )
    .unwrap();

    let _ = run_bpt_sign!(
        per_test_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock-songs.bbuild"),
        per_test_path!("fakeblock-song-gen.bbuild")
    )
    .unwrap();

    let stdout = run_bpt_verify!(
        per_test_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock-songs.bbuild"),
        per_test_path!("fakeblock-song-gen.bbuild")
    )
    .unwrap();
    assert!(stdout.contains("Verified all 3 file signatures"));
}

#[test]
#[named]
fn verify_missing_signature() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();

    let result = run_bpt_verify!(per_test_path!("fakeblock.bbuild"));
    assert!(result.is_err());
    let stdout = result.unwrap_err();
    assert!(stdout.contains("is not signed"));
}

#[test]
#[named]
fn verify_invalid_signature() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();

    const INVALID_SIGNATURE: &str = "\n# bpt-sig-v1:RURxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxM\n";
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(INVALID_SIGNATURE.as_bytes()).unwrap();
    drop(file);

    let result = run_bpt_verify!(per_test_path!("fakeblock.bbuild"));
    assert!(result.is_err());
    let stdout = result.unwrap_err();
    assert!(stdout.contains("No configured public key verifies"));
}

#[test]
#[named]
fn verify_corrupt_signature() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();

    const CORRUPT_SIGNATURE: &str = "\n# bpt-sig-v1:corrupt+signature\n";
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(CORRUPT_SIGNATURE.as_bytes()).unwrap();
    drop(file);

    let result = run_bpt_verify!(per_test_path!("fakeblock.bbuild"));
    assert!(result.is_err());
    let stdout = result.unwrap_err();
    assert!(stdout.contains("is corrupt"));
}

#[test]
#[named]
fn verify_multiple_fails_if_any_file_is_invalid() {
    setup_test!();

    std::fs::copy(
        repo_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-song-gen.bbuild"),
        per_test_path!("fakeblock-song-gen.bbuild"),
    )
    .unwrap();

    const CORRUPT_SIGNATURE: &str = "\n# bpt-sig-v1:corrupt+signature\n";
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock-song-gen.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(CORRUPT_SIGNATURE.as_bytes()).unwrap();
    drop(file);

    let result = run_bpt_verify!(
        per_test_path!("fakeblock.bbuild"),
        per_test_path!("fakeblock-song-gen.bbuild")
    );
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("is not signed") || stderr.contains("is corrupt"));
}

#[test]
#[named]
fn verify_missing_file_errors() {
    setup_test!();

    let result = run_bpt_verify!(per_test_path!("does-not-exist.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to open"));
    assert!(stderr.contains("does-not-exist.bbuild"));
}
