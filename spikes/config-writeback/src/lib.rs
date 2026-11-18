//! Throwaway spike for Settings M0.1: can allotd change a hand-edited `config.toml` without
//! losing anything the user wrote?
//!
//! Not part of the workspace and never shipped. M1.2 builds the real writer in `allot-shared`
//! from what this spike found, then deletes it.

pub mod edit;
pub mod file;
pub mod rules;
