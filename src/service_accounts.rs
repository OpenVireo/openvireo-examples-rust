//! Service accounts: create one for CI, then read, inject and write secrets with its token.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/service-accounts/
//!
//!     OPENVIREO_SERVICE_ACCOUNT_TOKEN=... cargo run -- service-accounts "ov://Team Infra/CI/token"
//!
//! create() needs an organisation owner or admin signed in, so run() doesn't call it. Set ROTATE to
//! a new value to also write it (needs the write role).
#![allow(dead_code)]

use ov_core::api::{Account, CancelToken, NewServiceAccount, ServiceAccount, VaultGrantRequest, VaultRole};

use crate::common::{env, make_client, text, usage, Result};

// Creating one
async fn create(account: &Account, org_id: String, vault_id: String, cancel: &CancelToken) -> Result {
    let input = NewServiceAccount {
        name: "ci".into(),
        vaults: vec![VaultGrantRequest { vault_id, role: VaultRole::Read }],
        expires_at: Some("2027-01-01T00:00:00Z".into()),
        secret_name: None,
    };
    let created = account.create_service_account(org_id, input, cancel.clone()).await?;
    println!("{}", text(&created.token)); // show it once; store it as a CI secret

    account
        .issue_service_account_secret(created.service_account.id, Some("staging".into()), None, cancel.clone())
        .await?;
    Ok(())
}

// Reading and writing
pub async fn run(args: Vec<String>) -> Result {
    let [reference] =
        <[String; 1]>::try_from(args).unwrap_or_else(|_| usage("Usage: service-accounts <ov://Vault/Entry/field>"));
    let cancel = CancelToken::new();
    let client = make_client("CI", false);
    let token = env("OPENVIREO_SERVICE_ACCOUNT_TOKEN").into_bytes();
    let sa = ServiceAccount::from_token(client, token, cancel.clone()).await?;
    let result = async {
        let password = sa.read(reference.clone(), cancel.clone()).await?;
        println!("{reference}: {} bytes", password.len());

        let template = format!("TOKEN={}\n", reference.replace(' ', "%20"));
        let injected = sa.inject(template.into_bytes(), cancel.clone()).await?; // mask injected.secrets in logs
        println!("injected {} secret(s)", injected.secrets.len());

        if let Ok(rotated) = std::env::var("ROTATE") {
            // needs the write role
            sa.write(reference, rotated.into_bytes(), false, cancel.clone()).await?;
        }
        Result::Ok(())
    }
    .await;
    sa.close(); // wipes the keys from memory
    result
}
