//! LAN play for the duel mode: one UDP socket per instance, a hello that
//! carries the player's name and the build identity, renet's channels on
//! top, timeouts, a link simulator for latency and loss tests, and LAN
//! discovery on the game port.
//!
//! The crate moves bytes only. What the bytes mean (inputs, snapshots,
//! events) belongs to the game; no game rule depends on this crate.

pub mod link;
pub mod session;
pub mod wire;

pub use session::{Browser, Event, Found, Peer, Role, Session, SessionConfig, Stats};

/// The UDP port a host listens on unless another is configured.
pub const DEFAULT_PORT: u16 = 47_620;

/// Channel ids: frequent state that may be lost, and events that must
/// arrive in order.
pub const UNRELIABLE: u8 = 0;
pub const RELIABLE: u8 = 1;
