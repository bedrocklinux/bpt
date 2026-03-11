//! Colors for printing to the terminal
//!
//! Public-facing interface is by concept (e.g. package name, success/failure) rather than actual
//! visible spectrum color (e.g. blue, green, etc).  This abstraction allows us to:
//!
//! - Enforce consistent color usage for a given concept across the code base.
//! - Update concept-color mapping by touching one place rather than every usage site.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

// This won't change during the program's execution, and so we can cache it.
static PRINT_COLOR: AtomicBool = AtomicBool::new(false);

// Call early (e.g. in main) to cache color decision.
// Respects NO_COLOR (https://no-color.org/) and FORCE_COLOR environment variables.
pub fn initialize_color() {
    let color = if std::env::var_os("NO_COLOR").is_some() {
        false
    } else if std::env::var_os("FORCE_COLOR").is_some() {
        true
    } else {
        std::io::stdout().is_terminal()
    };
    PRINT_COLOR.store(color, Ordering::Relaxed);
}

const DEFAULT: &str = "\x1b[0m";
const FG_BLUE: &str = "\x1b[0;38;5;33m";
const FG_CYAN: &str = "\x1b[0;36m";
const FG_GRAY: &str = "\x1b[0;90m";
const FG_GREEN: &str = "\x1b[0;32m";
const FG_LIGHT_MAGENTA: &str = "\x1b[0;95m";
const FG_LIGHT_RED: &str = "\x1b[1;31m";
const FG_LIGHT_YELLOW: &str = "\x1b[0;93m";
const FG_YELLOW: &str = "\x1b[0;33m";

pub enum Color {
    Default,
    Success,
    Error,
    Glue,
    Deemphasize,
    Match,
    Upgrade,
    Downgrade,
    Create,
    Remove,
    PkgName,
    PkgVer,
    Arch,
    Field,
    File,
    Url,
    Warn,
    Timestamp,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Only print colors when printing to a terminal
        if !PRINT_COLOR.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Map concept to color
        write!(
            f,
            "{}",
            match self {
                Color::Default => DEFAULT,
                Color::Success => FG_GREEN,
                Color::Error => FG_LIGHT_RED,
                Color::Glue => FG_GRAY,
                Color::Deemphasize => FG_GRAY,
                Color::Match => FG_GREEN,
                Color::Upgrade => FG_CYAN,
                Color::Downgrade => FG_YELLOW,
                Color::Create => FG_GREEN,
                Color::Remove => FG_LIGHT_RED,
                Color::PkgName => FG_CYAN,
                Color::PkgVer => FG_YELLOW,
                Color::Arch => FG_LIGHT_MAGENTA,
                Color::Field => FG_GREEN,
                Color::File => FG_GREEN,
                Color::Url => FG_BLUE,
                Color::Warn => FG_LIGHT_YELLOW,
                Color::Timestamp => FG_GRAY,
            }
        )
    }
}

/// Macro to handle color printing
///
/// Rust's [std::fmt::Display] does not differentiate between converting to a string (e.g.
/// `.to_string()`) and printing to the terminal (e.g. `println!()`).  This is problematic, as we
/// only want to print colors when printing to the terminal.
///
/// To resolve this, we retain the [std::fmt::Display] behavior for converting to a string, and use this
/// macro to generate a wrapper type whose [std::fmt::Display] includes color codes when stdout is a
/// terminal.  This wrapper type is accessible on the original via the method `.color()`.
#[macro_export]
macro_rules! make_display_color {
    ($type:ident, $color_fmt:expr) => {
        paste::paste! {
            // Color wrapper type
            pub struct [<Color $type>]<'a>(&'a $type);

            // Method to access the color wrapper type
            impl $type {
                pub fn color(&self) -> [<Color $type>]<'_> {
                    [<Color $type>](self)
                }
            }

            // Method to access the color wrapper's inner type
            impl [<Color $type>]<'_> {
                fn inner(&self) -> &$type {
                    &self.0
                }
            }

            // Color wrapper type's std::fmt::Display implementation
            impl std::fmt::Display for [<Color $type>]<'_> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let display_fn: &dyn Fn(&$type, &mut std::fmt::Formatter<'_>) -> std::fmt::Result = &$color_fmt;
                    display_fn(self.inner(), f)
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_color_macro() {
        initialize_color();

        struct TestStruct;

        impl std::fmt::Display for TestStruct {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "test")
            }
        }

        make_display_color!(TestStruct, |s, f| {
            write!(f, "{}{}{}", Color::Success, s, Color::Default)
        });

        let test = TestStruct;
        assert_eq!(format!("{}", test), "test");
        if PRINT_COLOR.load(Ordering::Relaxed) {
            assert_eq!(format!("{}", test.color()), "\x1b[0;32mtest\x1b[0m");
        } else {
            assert_eq!(format!("{}", test.color()), "test");
        }
    }
}
