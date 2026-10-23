//! Parent module for anything related to RAD Game Tools' Oodle compression format.

mod core;
mod bindings;
mod structs;

pub mod enums;

pub use core::*;
pub use bindings::*;
pub use structs::*;