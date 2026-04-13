//! Future daemon / service binary.
//!
//! `agentmemd` is intended to become the long-running local process that can:
//!
//! - coordinate multiple agent clients
//! - hold a warm in-memory index
//! - serialize writes safely
//! - expose local IPC / sockets later
//! - provide observability and health checks
//!
//! For now it is intentionally conservative and only boots, validates config,
//! opens the store, and waits until interrupted.

use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use agent_hashmap::cli::output::{
    print_error, print_field, print_heading, print_info, print_success,
};
use agent_hashmap::config::{resolve_local_config_path, Config};
use agent_hashmap::store::Store;

/// Poll interval while idle.
///
/// This is intentionally simple for v1 bootstrap mode.
const IDLE_INTERVAL: Duration = Duration::from_secs(5);

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(error) => {
            print_error(&error);
            1
        }
    };

    process::exit(code);
}

fn run() -> agent_hashmap::Result<()> {
    print_heading("agentmemd");
    print_info("starting local daemon");

    let config_path = resolve_local_config_path()?;
    let config = Config::load(&config_path)?;
    let store = Store::open_locked(config)?;

    let info = store.info();

    print_success("store opened");
    print_field("Project", info.project_name.as_str());
    print_field("Path", &info.path.to_string());
    print_field("Entries", &store.len().to_string());

    let running = Arc::new(AtomicBool::new(true));

    // Placeholder loop until real signal handling / IPC arrives.
    while running.load(Ordering::Relaxed) {
        thread::sleep(IDLE_INTERVAL);
    }

    Ok(())
}
