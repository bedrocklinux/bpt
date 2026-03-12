use crate::*;
use ::function_name::named;
use std::fs::FileTimes;
use std::io::{Seek, Write};
use std::time::SystemTime;

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_one() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Signed"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let _ = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_multiple() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-song-gen@1.0.0.bbuild"),
        per_test_path!("fakeblock-song-gen@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_sign!(
        per_test_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-song-gen@1.0.0.bbuild")
    )
    .unwrap();
    assert!(stdout.contains("Signed all 3 files"));

    let _ = run_bpt_verify!(
        per_test_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-song-gen@1.0.0.bbuild")
    )
    .unwrap();
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_already_signed() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let stdout = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Signed"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let stdout = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Verified"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let stdout = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Signed"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let _ = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_invalid_signature() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    const INVALID_SIGNATURE: &str = "\n# bpt-sig-v1:RURxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxM\n";
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(INVALID_SIGNATURE.as_bytes()).unwrap();
    drop(file);

    let result = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stdout = result.unwrap_err();
    assert!(stdout.contains("No configured public key verifies"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let stdout = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Signed"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let _ = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_corrupt_signature() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    const CORRUPT_SIGNATURE: &str = "\n# bpt-sig-v1:corrupt+signature\n";
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(CORRUPT_SIGNATURE.as_bytes()).unwrap();
    drop(file);

    let result = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stdout = result.unwrap_err();
    println!("{:?}", stdout);
    assert!(stdout.contains("Signature for"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));
    assert!(stdout.contains("is corrupt"));

    let stdout = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Signed"));
    assert!(stdout.contains("fakeblock@1.0.0.bbuild"));

    let _ = run_bpt_verify!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();
}

#[test]
#[named]
fn sign_missing_passphrase_file_errors() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();

    let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args([
            "-y",
            "-R",
            per_test_path!(),
            "-O",
            per_test_path!(),
            "-P",
            common_path!("etc/bpt/private-keys/test-key-password-is-bpt.key"),
            "--priv-key-passphrase-file",
            per_test_path!("does-not-exist.passphrase"),
            "sign",
            per_test_path!("fakeblock@1.0.0.bbuild"),
        ])
        .output()
        .expect("failed to execute bpt");
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("Unable to read"));
    assert!(stderr.contains("does-not-exist.passphrase"));
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_needed_skips_already_valid_file_without_decrypting_key() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    let _ = run_bpt_sign!(per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();

    let before = std::fs::metadata(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args([
            "-y",
            "-R",
            per_test_path!(),
            "-O",
            per_test_path!(),
            "sign",
            "--needed",
            per_test_path!("fakeblock@1.0.0.bbuild"),
        ])
        .output()
        .expect("failed to execute bpt");
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("All 1 files already had valid signatures"));
    assert!(!stdout.contains("Decrypting secret key"));

    let after = std::fs::metadata(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before, after, "sign --needed should not touch valid files");
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_needed_only_resigns_invalid_files() {
    setup_test!();
    std::fs::copy(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    std::fs::copy(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild"),
    )
    .unwrap();

    let valid_before = std::fs::metadata(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();
    let invalid_before = std::fs::metadata(per_test_path!("fakeblock-songs@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(per_test_path!("fakeblock-songs@1.0.0.bbuild"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    file.write_all(b"\n# bpt-sig-v1:corrupt+signature\n")
        .unwrap();
    file.set_times(FileTimes::new().set_modified(SystemTime::now()))
        .unwrap();
    drop(file);

    let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args([
            "-y",
            "-R",
            per_test_path!(),
            "-O",
            per_test_path!(),
            "-P",
            common_path!("etc/bpt/private-keys/test-key-password-is-bpt.key"),
            "--priv-key-passphrase-file",
            common_path!("etc/bpt/private-keys/test-key-password-is-bpt.passphrase"),
            "sign",
            "--needed",
            per_test_path!("fakeblock@1.0.0.bbuild"),
            per_test_path!("fakeblock-songs@1.0.0.bbuild"),
        ])
        .output()
        .expect("failed to execute bpt");
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Signed 1 of 2 files"));

    let valid_after = std::fs::metadata(per_test_path!("fakeblock@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();
    let invalid_after = std::fs::metadata(per_test_path!("fakeblock-songs@1.0.0.bbuild"))
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(
        valid_before, valid_after,
        "valid file should not be re-signed"
    );
    assert!(
        invalid_after >= invalid_before,
        "invalid file should be re-signed"
    );

    let _ = run_bpt_verify!(
        per_test_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@1.0.0.bbuild")
    )
    .unwrap();
}

#[ignore = "Decrypts test private key; slow."]
#[test]
#[named]
fn sign_missing_target_file_errors() {
    setup_test!();

    let result = run_bpt_sign!(per_test_path!("does-not-exist.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to open"));
    assert!(stderr.contains("does-not-exist.bbuild"));
}
