//! CLI command dispatcher.
//!
//! This module keeps binary entrypoints thin by handling:
//!
//! - argument parsing
//! - config resolution
//! - store opening
//! - command execution
//! - human-readable output
//!
//! Binaries should usually call `run()` and exit with the returned status code.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::cli::onboarding::run_onboarding;
use crate::cli::output::{
    print_blank_line, print_error, print_field, print_heading, print_info, print_line,
    print_success,
};
use crate::config::{resolve_local_config_path, Config};
use crate::error::Result;
use crate::index::{build_index, query_index, read_index_stats, QueryResult};
use crate::store::Store;
use crate::types::{Key, KeyPrefix, Namespace, Value};

/// Runs the CLI and returns a process exit code.
///
/// - `0` success
/// - `1` handled user-facing error
pub fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(error) => {
            print_error(&error);
            1
        }
    }
}

fn run_inner() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => {
            let _ = run_onboarding()?;
        }

        Command::Info => {
            let store = open_local_store()?;
            render_info(&store);
        }

        Command::Get { key } => {
            let store = open_local_store()?;
            let key = Key::new(key)?;

            if let Some(value) = store.get(&key) {
                print_line(value.as_str());
            } else {
                print_info("key not found");
            }
        }

        Command::Set { key, value } => {
            let mut store = open_local_store_locked()?;
            let key = Key::new(key)?;
            let value = Value::new(value)?;

            let _ = store.set(key, value)?;
            store.flush()?;

            print_success("value stored");
        }

        Command::Delete { key } => {
            let mut store = open_local_store_locked()?;
            let key = Key::new(key)?;

            match store.delete(&key) {
                Some(_) => {
                    store.flush()?;
                    print_success("key removed");
                }
                None => print_info("key not found"),
            }
        }

        Command::List { prefix } => {
            let store = open_local_store()?;

            match prefix {
                Some(prefix) => {
                    let prefix = KeyPrefix::new(prefix)?;
                    for entry in store.list_prefix(&prefix) {
                        print_line(format!("{} = {}", entry.key, entry.value));
                    }
                }
                None => {
                    for entry in store.entries() {
                        print_line(format!("{} = {}", entry.key, entry.value));
                    }
                }
            }
        }

        Command::Namespace { namespace } => {
            let store = open_local_store()?;
            let namespace = Namespace::new(namespace)?;

            for entry in store.list_namespace(&namespace) {
                print_line(format!("{} = {}", entry.key, entry.value));
            }
        }

        Command::Clear => {
            let confirmed = crate::cli::prompts::prompt_confirm("Delete all entries?", false)?;

            if confirmed {
                let mut store = open_local_store_locked()?;
                store.clear();
                store.flush()?;
                print_success("store cleared");
            } else {
                print_info("cancelled");
            }
        }

        Command::Index { command } => match command {
            IndexCommand::Build { root } => {
                let mut store = open_local_store_locked()?;
                let root = root.unwrap_or(std::env::current_dir()?);
                let report = build_index(&mut store, &root)?;

                print_success("index built");
                print_field("Root", report.root);
                print_field("Files", report.file_count.to_string());
                print_field("Skipped", report.skipped_files.to_string());
                print_field("Chunks", report.chunk_count.to_string());
                print_field("Tokens", report.token_count.to_string());
            }

            IndexCommand::Query {
                query,
                top_k,
                token_budget,
            } => {
                let store = open_local_store()?;
                let result = query_index(&store, &query, top_k, token_budget)?;
                render_index_query(&result);
            }

            IndexCommand::Stats => {
                let store = open_local_store()?;
                let stats = read_index_stats(&store);

                if !stats.built {
                    print_info("index not built");
                } else {
                    print_heading("Index Stats");
                    print_field("Root", stats.root.unwrap_or_default());
                    print_field("Files", stats.file_count.to_string());
                    print_field("Chunks", stats.chunk_count.to_string());
                    print_field("Tokens", stats.token_count.to_string());
                    print_field(
                        "Built (unix)",
                        stats
                            .built_unix_seconds
                            .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
                    );
                    print_blank_line();
                }
            }
        },
    }

    Ok(())
}

/// CLI root parser.
#[derive(Debug, Parser)]
#[command(
    name = "agentmem",
    version,
    about = "Secure local memory for AI agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Supported commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// First-run setup.
    Init,

    /// Show store information.
    Info,

    /// Get a value by key.
    Get {
        /// Validated key.
        key: String,
    },

    /// Set a value.
    Set {
        /// Validated key.
        key: String,

        /// Text value.
        value: String,
    },

    /// Delete a key.
    Delete {
        /// Validated key.
        key: String,
    },

    /// List entries, optionally filtered by prefix.
    List {
        /// Optional prefix filter.
        #[arg(long)]
        prefix: Option<String>,
    },

    /// List entries inside a namespace.
    Namespace {
        /// Namespace path.
        namespace: String,
    },

    /// Remove all entries.
    Clear,

    /// Build and query local code index.
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
}

/// Index subcommands.
#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Build or rebuild index for a workspace root.
    Build {
        /// Root directory to scan. Defaults to current directory.
        #[arg(long)]
        root: Option<PathBuf>,
    },

    /// Query ranked chunks from local index.
    Query {
        /// Natural language or keyword query.
        query: String,

        /// Maximum number of chunks to return.
        #[arg(long, default_value_t = 8)]
        top_k: usize,

        /// Maximum estimated tokens in returned chunks.
        #[arg(long, default_value_t = 4000)]
        token_budget: usize,
    },

    /// Show persisted index summary.
    Stats,
}

/// Opens the project-local store.
fn open_local_store() -> Result<Store> {
    let config_path = resolve_local_config_path()?;
    let config = Config::load(config_path)?;
    Store::open(config)
}

/// Opens the project-local store and acquires a lock.
fn open_local_store_locked() -> Result<Store> {
    let config_path = resolve_local_config_path()?;
    let config = Config::load(config_path)?;
    Store::open_locked(config)
}

/// Renders store information.
fn render_info(store: &Store) {
    let info = store.info();
    let stats = store.stats();

    print_heading("Store Info");
    print_field("Project", info.project_name.as_str());
    print_field("Path", &info.path.to_string());
    print_field("Entries", &stats.entry_count.to_string());
    print_field("Locked", if stats.locked { "yes" } else { "no" });
    print_field("Format", &info.format_version.to_string());

    print_blank_line();
}

fn render_index_query(result: &QueryResult) {
    print_heading("Index Query");
    print_field("Query", &result.query);
    print_field("Top K", result.top_k.to_string());
    print_field("Token budget", result.token_budget.to_string());
    print_field("Used tokens", result.used_tokens.to_string());
    print_field("Confidence", format!("{:.2}", result.confidence));
    print_field(
        "Fallback required",
        if result.fallback_required {
            "yes"
        } else {
            "no"
        },
    );
    print_field(
        "Matched tokens",
        if result.matched_tokens.is_empty() {
            "(none)".to_owned()
        } else {
            result.matched_tokens.join(", ")
        },
    );
    print_field(
        "Missing tokens",
        if result.missing_tokens.is_empty() {
            "(none)".to_owned()
        } else {
            result.missing_tokens.join(", ")
        },
    );
    print_blank_line();

    if result.chunks.is_empty() {
        print_info("no matching chunks");
        return;
    }

    for chunk in &result.chunks {
        print_line(format!(
            "{}:{}-{} score={} est_tokens={}",
            chunk.path, chunk.line_start, chunk.line_end, chunk.score, chunk.estimated_tokens
        ));
        print_line(truncate_for_terminal(&chunk.content, 700));
        print_blank_line();
    }
}

fn truncate_for_terminal(input: &str, max_len: usize) -> String {
    if input.len() <= max_len {
        return input.to_owned();
    }

    let mut clipped = input.chars().take(max_len).collect::<String>();
    clipped.push_str("...");
    clipped
}
