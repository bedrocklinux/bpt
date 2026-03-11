use crate::error::*;
use std::io::Write;

/// Confirmation prompt
pub fn confirm() -> Result<bool, Err> {
    print!("Continue? [y/N] ");
    Write::flush(&mut std::io::stdout()).map_err(Err::FlushStdout)?;

    let mut confirmation = String::new();
    std::io::stdin()
        .read_line(&mut confirmation)
        .map_err(|e| Err::Read("stdin".to_owned(), e))?;

    Ok(["y", "Y", "yes", "YES"].contains(&confirmation.trim_end()))
}
