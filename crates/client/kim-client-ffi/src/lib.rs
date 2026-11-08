#![allow(unexpected_cfgs)]

pub mod api;

#[allow(
    unsafe_code,
    unused_qualifications,
    unused,
    clippy::all,
    clippy::unwrap_used
)]
#[cfg(not(frb_expand))]
mod frb_generated;
