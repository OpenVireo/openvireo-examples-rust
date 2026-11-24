//! What every example shares: the issuer, environment variables, prompts, a client and a sign-in.

use std::io::Write;

use ov_core::api::{Account, CancelToken, ClientConfig, DeviceInfo, LoginInput, LoginStep, OvClient, Platform};

use crate::host::make_host;

/// What the examples return: any error, printed by main.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The account server: OV_ISSUER, or the hosted one.
pub fn issuer() -> String {
    std::env::var("OV_ISSUER").unwrap_or_else(|_| "https://accounts.openvireo.com".into())
}

/// A required environment variable; exits with a message when it isn't set.
pub fn env(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("Set {name} to run this example.");
            std::process::exit(1)
        }
    }
}

pub fn usage(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(2)
}

pub fn prompt(text: &str) -> String {
    print!("{text}");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    line.trim().to_owned()
}

pub fn ask_for_code() -> String {
    prompt("Authenticator code: ")
}

/// Secrets come back as bytes (UTF-8 for text).
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn make_client(name: &str, offline: bool) -> OvClient {
    OvClient::new(
        ClientConfig {
            device: DeviceInfo { id: None, name: name.into(), platform: Platform::Cli },
            offline,
            server: None,
        },
        make_host(),
    )
}

/// Connects and signs in from OV_EMAIL, OV_PASSWORD and OV_SECRET_KEY, asking for a TOTP code when
/// needed.
pub async fn sign_in(client: &OvClient, cancel: &CancelToken) -> Result<Account> {
    let account = client.connect(issuer(), cancel.clone()).await?;
    let input = LoginInput {
        email: env("OV_EMAIL"),
        password: env("OV_PASSWORD").into_bytes(),
        secret_key: std::env::var("OV_SECRET_KEY").ok().map(String::into_bytes),
        totp: None,
        trust_device: true,
    };
    let mut step = account.login(input, cancel.clone()).await?;
    while let LoginStep::NeedsTotp(challenge) = step {
        step = account.submit_totp(challenge, ask_for_code(), cancel.clone()).await?;
    }
    Ok(account)
}

/// Loads the core and reaches the account server, without signing in.
pub async fn check(_args: Vec<String>) -> Result {
    let account = make_client("OpenVireo examples", false).connect(issuer(), CancelToken::new()).await?;
    println!(
        "Connected to {} (API {}); signed in: {} (core {})",
        issuer(),
        account.server_config().api,
        account.is_signed_in(),
        ov_core::version()
    );
    Ok(())
}
