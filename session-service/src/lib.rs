#![no_std]
#![allow(clippy::crate_in_macro_def)]
pub use gstd::{exec, msg};
pub use schnorrkel::{PublicKey, Signature};

mod macros;
pub mod utils;
