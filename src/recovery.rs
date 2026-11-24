//! Account recovery: resetting with a recovery link, and restoring the old vaults afterwards.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/recovery/
//!
//!     cargo run -- recovery info <token>   # what a reset would do; works signed out
//!     cargo run -- recovery status         # after a reset: is there an archive to restore?
//!
//! reset() and restore() change the account, so they only run when you call them yourself.
#![allow(dead_code)]

use ov_core::api::{Account, CancelToken};

use crate::common::{issuer, make_client, sign_in, text, usage, Result};

// Resetting
async fn info(account: &Account, token: Vec<u8>, cancel: &CancelToken) -> Result {
    let info = account.recover_info(token, cancel.clone()).await?; // works signed out
    println!("{info:?}");
    Ok(())
}

async fn reset(account: &Account, token: Vec<u8>, new_password: Vec<u8>, cancel: &CancelToken) -> Result {
    let created = account.recover(token, new_password, None, true, cancel.clone()).await?;
    println!("Your new Secret Key (keep it safe): {}", text(&created.secret_key)); // signed in and unlocked afterwards
    Ok(())
}

// Restoring the old vaults
async fn restore(account: &Account, old_password: Vec<u8>, old_secret_key: Vec<u8>, cancel: &CancelToken) -> Result {
    let status = account.recovery_status(cancel.clone()).await?;
    if status.archived && !status.locked {
        let restored = account.restore_from_archive(old_password, old_secret_key, cancel.clone()).await?;
        println!("restored {restored} vault(s)"); // vault list reloaded
    }
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    let cancel = CancelToken::new();
    let client = make_client("OpenVireo examples", false);
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["info", token] => {
            let account = client.connect(issuer(), cancel.clone()).await?;
            info(&account, token.as_bytes().to_vec(), &cancel).await
        }
        ["status"] => {
            let account = sign_in(&client, &cancel).await?;
            println!("{:?}", account.recovery_status(cancel.clone()).await?);
            account.sign_out(false, cancel).await?;
            Ok(())
        }
        _ => usage("Usage: recovery info <token> | recovery status"),
    }
}
