//! Hosted accounts: connecting, signing up and in, locking, the account itself, devices and deletion.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/hosted-account/
//!
//! Running it signs in, prints the account and its devices, locks and unlocks, and signs out. The
//! functions that change the account (sign_up, change_password, delete_account, ...) are here to
//! read and copy; run() doesn't call them.
//!
//!     cargo run -- hosted-account
#![allow(dead_code)]

use ov_core::api::{
    Account, CancelToken, ClientConfig, DeviceInfo, LoginInput, LoginStep, OvClient, Platform, RegisterInput,
};

use crate::common::{ask_for_code, env, issuer, prompt, text, Result};
use crate::host::make_host;

// Connecting
async fn connect(cancel: &CancelToken) -> Result<(OvClient, Account)> {
    // make_host(): your Host (HTTP, a keychain-backed SecureStore, a cache, a clock, a log sink)
    let client = OvClient::new(
        ClientConfig {
            device: DeviceInfo { id: None, name: "Laptop".into(), platform: Platform::Cli },
            offline: false,
            server: None,
        },
        make_host(),
    );
    let account = client.connect(issuer(), cancel.clone()).await?;
    Ok((client, account))
}

// Signing up
async fn sign_up(account: &Account, email: String, password: Vec<u8>, cancel: &CancelToken) -> Result {
    let created =
        account.register(RegisterInput { email, password, kdf: None, trust_device: true }, cancel.clone()).await?;
    show_emergency_kit(&created.secret_key); // the only time the Secret Key is shown; the user must keep it
    Ok(())
}

fn show_emergency_kit(secret_key: &[u8]) {
    println!("Your Secret Key (keep it safe): {}", text(secret_key));
}

// Signing in
async fn sign_in(
    account: &Account,
    email: String,
    password: Vec<u8>,
    secret_key: Option<Vec<u8>>, // only needed the first time on this device
    cancel: &CancelToken,
) -> Result {
    let input = LoginInput { email, password, secret_key, totp: None, trust_device: true };
    let mut step = account.login(input, cancel.clone()).await?;
    while let LoginStep::NeedsTotp(challenge) = step {
        // two-factor authentication is on
        step = account.submit_totp(challenge, ask_for_code(), cancel.clone()).await?;
    }
    Ok(())
}

// Unlocking and locking
async fn lock_and_unlock(account: &Account, password: Vec<u8>, cancel: &CancelToken) -> Result {
    account.lock(); // wipes the keys from memory; the session stays
    if account.is_signed_in() && !account.is_unlocked() {
        account.unlock(password, None, cancel.clone()).await?;
    }
    Ok(())
}

async fn sign_out(account: &Account, forget_device: bool, cancel: &CancelToken) -> Result {
    // false ends the session and forgets it here; true also forgets the trusted Secret Key
    account.sign_out(forget_device, cancel.clone()).await?;
    Ok(())
}

// Staying unlocked across a relaunch
async fn stay_unlocked(account: &Account, reloaded: &Account, cancel: &CancelToken) -> Result {
    let secret = account.unlocked_keyset_secret()?; // keep it in the keychain

    // after a relaunch, on a connected (signed-in, locked) account
    reloaded.resume_unlocked(secret, cancel.clone()).await?;
    Ok(())
}

async fn watch_for_sign_out(account: &Account, cancel: &CancelToken) -> Result {
    let reason = account.signed_out(cancel.clone()).await?; // SessionExpired, TokenReused, Unauthorized, Logout, ...
    println!("Signed out: {reason:?}");
    Ok(())
}

// The account itself
fn show_account(account: &Account) -> Result {
    println!("email: {}", account.email().unwrap_or_default());
    println!("user id: {}", account.user_id().unwrap_or_default());
    println!("fingerprint: {}", account.fingerprint()?); // this account's public key fingerprint, for others to compare
    Ok(())
}

async fn change_password(account: &Account, current: Vec<u8>, next: Vec<u8>, cancel: &CancelToken) -> Result {
    account.change_password(current, next, None, cancel.clone()).await?; // signs others out
    Ok(())
}

async fn change_email(account: &Account, new_email: String, password: Vec<u8>, cancel: &CancelToken) -> Result {
    account.request_email_change(new_email.clone(), cancel.clone()).await?; // mails a code
    let code = prompt(&format!("Code sent to {new_email}: "));
    account.change_email(new_email, code, password, cancel.clone()).await?;
    Ok(())
}

async fn account_website_link(account: &Account, cancel: &CancelToken) -> Result<String> {
    let url = account.web_session_url(Some("/settings".into()), cancel.clone()).await?; // one-time (60 s)
    Ok(text(&url))
}

// Devices
async fn list_devices(account: &Account, cancel: &CancelToken) -> Result {
    for d in account.devices(cancel.clone()).await? {
        // name, platform, last_seen_at, current, ...
        println!("{} {} {} {}", d.name, d.platform, d.last_seen_at, if d.current { "(this device)" } else { "" });
    }
    Ok(())
}

async fn revoke_device(account: &Account, name: &str, cancel: &CancelToken) -> Result {
    let devices = account.devices(cancel.clone()).await?;
    if let Some(old) = devices.iter().find(|d| !d.current && d.name == name) {
        account.revoke_device(old.id.clone(), cancel.clone()).await?;
    }
    Ok(())
}

// Deleting the account
async fn delete_account(
    account: &Account,
    password: Vec<u8>,
    typed_email: String,
    heir: Option<String>,
    cancel: &CancelToken,
) -> Result {
    let preview = account.deletion_preview(cancel.clone()).await?;
    println!("would be erased on {}", preview.erase_on);
    for v in &preview.vaults {
        println!("vault {} shared with {}", v.name.as_deref().unwrap_or("?"), v.shared_with.len());
        // shared vaults can be given away first
    }
    for o in &preview.orgs {
        println!("org {}: {:?}", o.name, o.outcome); // leave, delete or blocked
    }
    if let (Some(heir), Some(vault)) = (heir, preview.vaults.first()) {
        if !vault.shared_with.is_empty() {
            account.transfer_vault(vault.id.clone(), heir, cancel.clone()).await?;
        }
    }

    let scheduled_for = account.delete_account(password.clone(), typed_email, cancel.clone()).await?;
    println!("scheduled for {scheduled_for}");
    // This device is signed out and its offline cache is gone. The Secret Key stays, so after signing in again:
    if account.deletion_scheduled_for().is_some() {
        account.restore_account(password, cancel.clone()).await?;
    }
    Ok(())
}

pub async fn run(_args: Vec<String>) -> Result {
    let cancel = CancelToken::new();
    let password = env("OV_PASSWORD").into_bytes();
    let (_client, account) = connect(&cancel).await?;
    let secret_key = std::env::var("OV_SECRET_KEY").ok().map(String::into_bytes);
    sign_in(&account, env("OV_EMAIL"), password.clone(), secret_key, &cancel).await?;
    show_account(&account)?;
    list_devices(&account, &cancel).await?;
    lock_and_unlock(&account, password, &cancel).await?;
    sign_out(&account, false, &cancel).await
}
