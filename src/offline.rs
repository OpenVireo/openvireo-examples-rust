//! Offline: edit from the cached copy while the servers can't be reached, and merge the edits in
//! when back online.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/offline/
//!
//! To unlock offline after a restart, the Host needs a persistent SecureStore (the keychain) and
//! BlobCache (files). The examples' Host keeps both in memory, so this program signs in (online)
//! and then edits, keeping the edit if the network drops.
//!
//!     cargo run -- offline "ov://Home/Wifi/password" "new value"

use ov_core::api::{CancelToken, SyncOptions};
use ov_core::OvErrorCode;

use crate::common::{make_client, sign_in, usage, Result};

pub async fn run(args: Vec<String>) -> Result {
    let [reference, value] =
        <[String; 2]>::try_from(args).unwrap_or_else(|_| usage("Usage: offline <ov://Vault/Entry/field> <value>"));
    let cancel = CancelToken::new();
    let client = make_client("Laptop", true); // offline needs a Host with a persistent BlobCache
    let account = sign_in(&client, &cancel).await?; // with one: client.connect, then account.unlock(password, None, cancel)
    let vault_name = reference.trim_start_matches("ov://").split('/').next().unwrap_or_default().to_owned();
    let Some(info) = account.vaults(cancel.clone()).await?.into_iter().find(|v| v.name == vault_name) else {
        usage(&format!("No vault named {vault_name}"));
    };
    let vault = account.open_vault(info.id, cancel.clone()).await?; // offline: the local copy
    println!("offline: {}", account.is_offline()); // true while unreachable

    match vault.write_field(reference, value.into_bytes(), false, cancel.clone()).await {
        Ok(_) => {}
        // offline: the edit is kept and merges in later
        Err(e) if e.code == OvErrorCode::NetworkError => println!("Saved locally; it uploads when you're back online."),
        Err(e) => return Err(e.into()),
    }

    if account.is_offline() {
        let result = account.reconnect(cancel.clone()).await?; // back online; result.dropped: revoked vaults
        println!("reconnected; dropped: {:?}", result.dropped);
        vault.sync(SyncOptions { local_changed: true, accept_server_state: false }, cancel.clone()).await?;
    }
    account.sign_out(false, cancel).await?;
    Ok(())
}
