//! vynamp-engine
//!
//! Especifico deste modulo: ver specs/ no proprio repo.

/// Versao do modulo. O contrato de release do umbrella fixa a tag.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod playback;

pub use playback::{
    CancelToken, Command, CommandKind, Engine, Error, Event, Generation, PlaybackState,
};
