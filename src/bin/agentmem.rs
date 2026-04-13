//! Main CLI binary for Agent Memory RS.
//!
//! This binary intentionally stays minimal.
//! All substantial logic belongs in the library crate so it can be:
//!
//! - tested
//! - reused
//! - embedded by other tools
//! - kept separate from process concerns

use std::process;

fn main() {
    let code = agent_hashmap::cli::run();
    process::exit(code);
}
