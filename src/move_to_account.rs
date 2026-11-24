//! Moving a .kdbx file into an account: as a new vault, or merged into a vault you already have.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/move-to-account/
//!
//!     cargo run -- move-to-account Passwords.kdbx            # a new vault named after the file
//!     cargo run -- move-to-account Passwords.kdbx Personal   # merge into the vault "Personal"
//!
//! The file's password is read from KDBX_PASSWORD.

use std::path::Path;

use ov_core::api::{Account, CancelToken, ImportMode, KdbxCredentials, LocalImportMode, OvClient, SyncOptions, Vault};

use crate::common::{env, make_client, sign_in, usage, Result};

async fn import_as_new_vault(
    account: &Account,
    file: Vec<u8>,
    password: Vec<u8>,
    name: String,
    cancel: &CancelToken,
) -> Result {
    let creds = KdbxCredentials { password: Some(password), ..Default::default() };
    let outcome = account
        .import_kdbx(file, creds, ImportMode::NewVault { name: Some(name), org_id: None }, cancel.clone())
        .await?;
    if !outcome.uploaded {
        // the vault exists but its first upload failed (outcome.upload_error): retry it later
        outcome.vault.sync(SyncOptions { local_changed: true, accept_server_state: false }, cancel.clone()).await?;
    }
    println!("Imported into {}", outcome.vault.name());
    Ok(())
}

async fn merge_into(
    client: &OvClient,
    vault: &Vault,
    file: Vec<u8>,
    password: Vec<u8>,
    cancel: &CancelToken,
) -> Result {
    let creds = KdbxCredentials { password: Some(password), ..Default::default() };
    let source = client.open_local(file, creds, cancel.clone()).await?;
    // Merge (by UUID), or LocalImportMode::Copies { parent_group_id }, with vault.import_targets() listing the groups to offer
    let summary = vault.import_from(source, LocalImportMode::Merge)?;
    vault.sync(SyncOptions { local_changed: true, accept_server_state: false }, cancel.clone()).await?;
    println!("{summary:?}"); // added, updated, unchanged
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    if !(1..=2).contains(&args.len()) {
        usage("Usage: move-to-account <file.kdbx> [vault name]");
    }
    let path = Path::new(&args[0]);
    let file = std::fs::read(path)?;
    let password = env("KDBX_PASSWORD").into_bytes();
    let cancel = CancelToken::new();
    let client = make_client("OpenVireo examples", false);
    let account = sign_in(&client, &cancel).await?;
    if let Some(vault_name) = args.get(1) {
        match account.vaults(cancel.clone()).await?.into_iter().find(|v| &v.name == vault_name) {
            Some(info) => {
                let vault = account.open_vault(info.id, cancel.clone()).await?;
                merge_into(&client, &vault, file, password, &cancel).await?;
            }
            None => println!("No vault named {vault_name}"),
        }
    } else {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        import_as_new_vault(&account, file, password, name, &cancel).await?;
    }
    account.sign_out(false, cancel).await?;
    Ok(())
}
