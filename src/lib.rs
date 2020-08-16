#![forbid(unsafe_code)]

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "sms")]
pub mod sms;

#[cfg(feature = "location")]
pub mod location;
