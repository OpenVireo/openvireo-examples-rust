//! A self-hosted sync server: pairing a device, syncing a .kdbx file, and the other calls.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/self-hosted-sync/
//!
//!     cargo run -- self-hosted-sync https://sync.example.org ABCD-EFGH personal Passwords.kdbx
//!
//! Pairs with the server, syncs the file (password from KDBX_PASSWORD; the merged file is written
//! back to the same path) and shows the other calls. The examples' Host keeps the pairing in memory;
//! with a keychain-backed SecureStore, SelfHosted::restore finds it again next launch.

use std::path::Path;

use ov_core::api::{CancelToken, KdbxCredentials, OvClient, SelfHosted};

use crate::common::{env, make_client, usage, Result};

// Pairing
async fn pair(client: &OvClient, url: String, code: String, cancel: &CancelToken) -> Result<SelfHosted> {
    Ok(SelfHosted::pair(client.clone(), url, code, "Laptop".into(), cancel.clone()).await?)
}

async fn restore(client: &OvClient, url: String) -> Result<Option<SelfHosted>> {
    Ok(SelfHosted::restore(client.clone(), url).await?) // next time, with the same persistent store
}

// Syncing a database
async fn sync_file(
    server: &SelfHosted,
    database_id: String,
    path: &Path,
    password: Vec<u8>,
    cancel: &CancelToken,
) -> Result {
    let creds = KdbxCredentials { password: Some(password), ..Default::default() };
    let result = server.sync_database(database_id, std::fs::read(path)?, creds, cancel.clone()).await?;
    // result.kdbx is the merged file: save it locally and reopen it (result.report says what changed)
    std::fs::write(path, &result.kdbx)?;
    println!("{:?}", result.report);
    Ok(())
}

// Other calls
async fn other_calls(server: &SelfHosted, cancel: &CancelToken) -> Result {
    println!("{}", server.health(cancel.clone()).await?); // '{"status":"ok"}'
    for db in server.list_databases(cancel.clone()).await? {
        println!("{} {} {} {}", db.id, db.revision, db.size, db.updated_at);
    }
    let first = server.download("personal".into(), cancel.clone()).await?; // the .kdbx, or None
    println!("personal: {}", first.map_or("none".into(), |b| format!("{} bytes", b.len())));
    let code = server.create_pairing_code(cancel.clone()).await?.code; // for the next device
    println!("pairing code for the next device: {code}");
    let stream = server.events(cancel.clone()).await?; // next() gives AccountEvent::DatabaseChanged
    stream.close();
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    let [url, code, database_id, file] = <[String; 4]>::try_from(args)
        .unwrap_or_else(|_| usage("Usage: self-hosted-sync <server url> <pairing code> <database id> <file.kdbx>"));
    let cancel = CancelToken::new();
    let client = make_client("Laptop", false);
    let server = match restore(&client, url.clone()).await? {
        Some(restored) => restored,
        None => pair(&client, url, code, &cancel).await?,
    };
    sync_file(&server, database_id, Path::new(&file), env("KDBX_PASSWORD").into_bytes(), &cancel).await?;
    other_calls(&server, &cancel).await
}
