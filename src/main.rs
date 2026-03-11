// bpt/main.rs
//
//      This program is free software; you can redistribute it and/or
//      modify it under the terms of the GNU General Public License
//      version 2 as published by the Free Software Foundation.
//
// Copyright (c) 2022-2026 Daniel Thau <danthau@bedrocklinux.org>
//
//! Bedrock Package Tool
//!
//! Bedrock Package Tool (bpt) is a package manager for managing bedrock strata within Bedrock Linux
//! (<http://bedrocklinux.org>) systems.

mod cli;
mod collection;
mod color;
mod command;
mod constant;
mod error;
mod file;
mod io;
mod location;
mod marshalling;
mod metadata;
mod reconcile;
mod str;
#[cfg(test)]
mod testutil;

fn main() {
    crate::color::initialize_color();

    use color::Color::*;

    match <cli::Cli as clap::Parser>::parse().run() {
        Ok(msg) if msg.is_empty() => {}
        Ok(msg) => println!("{Success}{msg}{Default}"),
        Err(err) => {
            eprintln!("{Error}ERROR: {err}{Default}");
            std::process::exit(err.exit_code())
        }
    }
}
