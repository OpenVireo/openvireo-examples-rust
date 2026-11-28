//! Runnable examples for the OpenVireo Rust core (`openvireo-core`, library `ov_core`), one module per guide on
//! developer.openvireo.com.
//!
//!     cargo run -- <example> [args]

mod common;
mod host;

mod events;
mod first_program;
mod hosted_account;
mod move_to_account;
mod offline;
mod recovery;
mod self_hosted_sync;
mod service_accounts;
mod share_links;
mod vaults_and_sharing;

use std::future::Future;
use std::pin::Pin;

type Run = fn(Vec<String>) -> Pin<Box<dyn Future<Output = common::Result>>>;

/// The examples, by the name you pass to `cargo run -- <name> ...`.
const EXAMPLES: &[(&str, Run)] = &[
    ("first-program", |a| Box::pin(first_program::run(a))),
    ("hosted-account", |a| Box::pin(hosted_account::run(a))),
    ("vaults-and-sharing", |a| Box::pin(vaults_and_sharing::run(a))),
    ("events", |a| Box::pin(events::run(a))),
    ("offline", |a| Box::pin(offline::run(a))),
    ("move-to-account", |a| Box::pin(move_to_account::run(a))),
    ("recovery", |a| Box::pin(recovery::run(a))),
    ("self-hosted-sync", |a| Box::pin(self_hosted_sync::run(a))),
    ("service-accounts", |a| Box::pin(service_accounts::run(a))),
    ("share-links", |a| Box::pin(share_links::run(a))),
    ("check", |a| Box::pin(common::check(a))),
];

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    let Some((_, run)) = EXAMPLES.iter().find(|(n, _)| *n == name) else {
        let names: Vec<_> = EXAMPLES.iter().map(|(n, _)| *n).collect();
        common::usage(&format!("Usage: cargo run -- <example> [args]\nExamples: {}", names.join(", ")));
    };
    if let Err(e) = run(args.collect()).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
