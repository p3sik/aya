//! An eBPF object file parsing library with BTF and relocation support.
//!
//! # Status
//!
//! This crate includes code that started as internal API used by
//! the [aya] crate. It has been split out so that it can be used by
//! other projects that deal with eBPF object files. Unless you're writing
//! low level eBPF plumbing tools, you should not need to use this crate
//! but see the [aya] crate instead.
//!
//! The API as it is today has a few rough edges and is generally not as
//! polished nor stable as the main [aya] crate API. As always,
//! improvements welcome!
//!
//! [aya]: https://github.com/aya-rs/aya
//!
//! # Overview
//!
//! eBPF programs written with [libbpf] or [aya-bpf] are usually compiled
//! into an ELF object file, using various sections to store information
//! about the eBPF programs.
//!
//! `aya-obj` is a library for parsing such eBPF object files, with BTF and
//! relocation support.
//!
//! [libbpf]: https://github.com/libbpf/libbpf
//! [aya-bpf]: https://github.com/aya-rs/aya
//!
//! # Example
//!
//! This example loads a simple eBPF program and runs it with [rbpf].
//!
//! ```no_run
//! use aya_obj::{generated::bpf_insn, Object};
//!
//! // Parse the object file
//! let bytes = std::fs::read("program.o").unwrap();
//! let mut object = Object::parse(&bytes).unwrap();
//! // Relocate the programs
//! object.relocate_calls().unwrap();
//! object.relocate_maps(std::iter::empty()).unwrap();
//!
//! // Run with rbpf
//! let instructions = &object.programs["prog_name"].function.instructions;
//! let data = unsafe {
//!     core::slice::from_raw_parts(
//!         instructions.as_ptr() as *const u8,
//!         instructions.len() * core::mem::size_of::<bpf_insn>(),
//!     )
//! };
//! let vm = rbpf::EbpfVmNoData::new(Some(data)).unwrap();
//! let _return = vm.execute_program().unwrap();
//! ```
//!
//! [rbpf]: https://github.com/qmonnet/rbpf

#![no_std]
#![doc(
    html_logo_url = "https://aya-rs.dev/assets/images/crabby.svg",
    html_favicon_url = "https://aya-rs.dev/assets/images/crabby.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(clippy::all, missing_docs)]
#![allow(clippy::missing_safety_doc, clippy::len_without_is_empty)]
#![cfg_attr(feature = "no_std", feature(error_in_core))]

#[cfg(feature = "no_std")]
pub(crate) use thiserror_core as thiserror;
#[cfg(not(feature = "no_std"))]
pub(crate) use thiserror_std as thiserror;

extern crate alloc;
#[cfg(not(feature = "no_std"))]
extern crate std;

pub mod btf;
pub mod generated;
pub mod maps;
pub mod obj;
pub mod programs;
pub mod relocation;
mod util;

pub use maps::Map;
pub use obj::*;
