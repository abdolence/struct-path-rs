//! A helper macros implementation to build a string that represents struct fields path at compile time.
//!
//! Example:
//! ```
//! use struct_path::*;
//!
//! ```
//!

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::*;
use syn::*;

