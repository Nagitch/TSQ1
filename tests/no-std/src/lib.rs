//! Compile-only integration check for TSQ1 without its default `std` feature.

#![no_std]

pub use tsq1::{convert_midi_to_tsq_vec, convert_tsq_to_midi_vec, Error};
