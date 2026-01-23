//! UCI Protocol Implementation
//!
//! Provides UCI (Universal Chess Interface) support for communication with
//! chess GUIs like Arena, Cutechess, and Lichess bots.

mod handler;
mod protocol;

pub use handler::UciEngine;
pub use protocol::{
    format_bestmove, format_info, format_uci_id, move_to_uci, parse_command, parse_uci_move,
    GoParams, UciCommand,
};
