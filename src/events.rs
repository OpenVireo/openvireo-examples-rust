//! Live events: react when another device changes a vault, or when access changes.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/events/
//!
//! Running it signs in, opens the first vault and prints events until the session ends (Ctrl-C to
//! stop). Edit something in another OpenVireo app to see one.
//!
//!     cargo run -- events

use ov_core::api::{Account, AccountEvent, CancelToken, SyncOptions, Vault, VaultInfo};

use crate::common::{make_client, sign_in, Result};

fn render(vaults: &[VaultInfo]) {
    let names: Vec<_> = vaults.iter().map(|v| v.name.as_str()).collect();
    println!("Vaults now: {}", names.join(", "));
}

async fn watch(account: &Account, vault: &Vault, cancel: &CancelToken) -> Result {
    let stream = account.subscribe(cancel.clone()).await?;
    while let Some(event) = stream.next(cancel.clone()).await? {
        match event {
            AccountEvent::VaultChanged { vault_id, etag, .. } => {
                // another device uploaded (or deleted) a vault this account can open
                println!("Vault changed: {vault_id}");
                if vault_id == vault.id() && etag != vault.etag() {
                    vault
                        .sync(SyncOptions { local_changed: false, accept_server_state: false }, cancel.clone())
                        .await?;
                }
            }
            AccountEvent::VaultsChanged { vaults } => render(&vaults), // access changed
            AccountEvent::SessionEnded { .. } => break,
            _ => {}
        }
    }
    stream.close(); // from any thread; a pending next() then returns None
    Ok(())
}

pub async fn run(_args: Vec<String>) -> Result {
    let cancel = CancelToken::new();
    let account = sign_in(&make_client("OpenVireo examples", false), &cancel).await?;
    match account.vaults(cancel.clone()).await?.first() {
        Some(first) => {
            println!("Watching for changes (Ctrl-C to stop)...");
            let vault = account.open_vault(first.id.clone(), cancel.clone()).await?;
            watch(&account, &vault, &cancel).await?;
        }
        None => println!("No vaults to watch."),
    }
    account.sign_out(false, cancel).await?;
    Ok(())
}
