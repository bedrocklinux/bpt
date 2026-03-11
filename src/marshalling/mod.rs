//! Marshalling
//!
//! Most bpt files are roughly structured in the following fashion:
//!
//!     [magic number] [timestamp] [block 1] [00] [block 2] [00] ... [block n] [signature]
//!
//! where blocks are structured in the following fashion:
//!
//!     [field 1] [0] [field 2] [0] ... [field n] [0]
//!
//! This module contains code to handle marshalling of these components.
//!
//! See [crate::metadata] for the fields which are marshalled.

mod block;
mod field;
mod field_list;
mod field_str;
mod magic_number;
pub use block::*;
pub use field::*;
pub use field_list::*;
pub use field_str::*;
pub use magic_number::*;
