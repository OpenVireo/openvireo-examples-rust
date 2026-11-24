//! A first program: sign in, list the vaults, read one field and sign out.
//!
//! Guide: https://developer.openvireo.com/sdk/#a-first-program
//!
//!     cargo run -- first-program "ov://Personal/GitHub/username"

use ov_core::api::{CancelToken, ClientConfig, DeviceInfo, LoginInput, OvClient, Platform};

use crate::common::{env, issuer, Result};
use crate::host::make_host;

async fn first_program(email: String, password: String, secret_key: String, reference: Option<String>) -> Result {
    // The Host is the platform's part: HTTP, a secure store, a cache, a clock and a log sink (host.rs)
    let client = OvClient::new(
        ClientConfig {
            device: DeviceInfo { id: None, name: "My script".into(), platform: Platform::Cli },
            offline: false,
            server: None,
        },
        make_host(),
    );
    let cancel = CancelToken::new();
    let account = client.connect(issuer(), cancel.clone()).await?;
    // The LoginStep asks for a TOTP code when two-step sign-in is on: see hosted_account.rs
    account
        .login(
            LoginInput {
                email,
                password: password.into_bytes(),
                secret_key: Some(secret_key.into_bytes()),
                totp: None,
                trust_device: true,
            },
            cancel.clone(),
        )
        .await?;

    let vaults = account.vaults(cancel.clone()).await?;
    for info in &vaults {
        println!("{} {:?}", info.name, info.role);
    }
    if let (Some(first), Some(reference)) = (vaults.first(), reference) {
        let vault = account.open_vault(first.id.clone(), cancel.clone()).await?;
        println!("{}", String::from_utf8_lossy(&vault.read_field(reference)?));
    }
    account.sign_out(false, cancel).await?;
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    first_program(env("OV_EMAIL"), env("OV_PASSWORD"), env("OV_SECRET_KEY"), args.into_iter().next()).await
}
