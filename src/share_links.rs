//! Share links: share one item, or some text, through a link that needs no account; list and revoke
//! links; open one.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/share-links/
//!
//!     cargo run -- share-links list
//!     cargo run -- share-links text "The alarm code is 4321"
//!     cargo run -- share-links open "https://share.openvireo.com/<id>#<key>"
#![allow(dead_code)]

use ov_core::api::{
    default_share_fields, item_share_payload, parse_totp_uri, shareable_fields, text_share_payload, totp_code, Account,
    CancelToken, OvClient, ShareLinkPolicy, ShareOptions, Vault,
};

use crate::common::{make_client, prompt, sign_in, text, usage, Result};

// Making a link
async fn share_entry(account: &Account, vault: &Vault, entry_id: String, cancel: &CancelToken) -> Result<String> {
    let entry = vault.entry(entry_id.clone())?;
    let choices = shareable_fields(entry.clone()); // what can be shared, in display order
    println!("can share: {:?}", choices.iter().map(|f| &f.name).collect::<Vec<_>>());

    let payload = item_share_payload(entry, default_share_fields(), Some("Sam".into())); // username, password, website
    let options = ShareOptions {
        expires_in: 7 * 86400, // seconds: 1 hour to 30 days (free plans: 7 days)
        max_opens: Some(1),    // None: until it expires
        passcode: None,
        vault_id: Some(vault.id()),
        entry_id: Some(entry_id),
    };
    let link = account.create_share(payload, options, cancel.clone()).await?;
    Ok(text(&link.url)) // https://share.openvireo.com/<id>#<key>
}

async fn share_text(account: &Account, message: &str, cancel: &CancelToken) -> Result<String> {
    let payload = text_share_payload(message.as_bytes().to_vec(), Some("From the examples".into()), None);
    let options = ShareOptions {
        expires_in: 3600,
        max_opens: None,
        passcode: Some(b"tiger lily 42".to_vec()),
        vault_id: None,
        entry_id: None,
    };
    let link = account.create_share(payload, options, cancel.clone()).await?;
    Ok(text(&link.url))
}

// Listing and revoking
async fn list_shares(account: &Account, cancel: &CancelToken) -> Result {
    for s in account.list_shares(cancel.clone()).await? {
        // s.reference has vault, entry, title, readable only by this account
        let title = s.reference.as_ref().map(|r| r.title.clone()).unwrap_or_default();
        println!(
            "{} {title} {} {}/{} {} {}",
            s.id,
            s.status,
            s.opens,
            s.max_opens.unwrap_or(0),
            s.expires_at,
            s.end_reason.unwrap_or_default()
        );
    }
    Ok(())
}

async fn revoke(account: &Account, share_id: String, cancel: &CancelToken) -> Result {
    account.revoke_share(share_id, cancel.clone()).await?;
    Ok(())
}

async fn org_policy(account: &Account, org_id: String, cancel: &CancelToken) -> Result {
    account.set_org_share_links(org_id, ShareLinkPolicy::Managers, cancel.clone()).await?; // On (default), Managers, Off
    Ok(())
}

// Opening a link
async fn open_link(client: &OvClient, api: String, link: String, cancel: &CancelToken) -> Result {
    let share_id = ov_share::parse_share_link(&link)?.id;
    let info = client.share_info(api.clone(), share_id, cancel.clone()).await?;
    // Show a "Reveal" button and open only when it's pressed: link previews then can't use up a one-time link.
    prompt("Press Enter to reveal... ");
    let passcode = info.kdf.is_some().then(|| prompt("Passcode: ").into_bytes());
    let opened = client.open_share(api, link.into_bytes(), passcode, Some(info), cancel.clone()).await?;

    for f in opened.payload.fields {
        let shown = if f.k == "totp" {
            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64;
            text(&totp_code(parse_totp_uri(f.value)?, now_ms).code)
        } else {
            text(&f.value)
        };
        // render as plain text; hide f.protected values until shown
        println!("{}: {shown}", f.label);
    }
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    let cancel = CancelToken::new();
    let client = make_client("OpenVireo examples", false);
    let account = sign_in(&client, &cancel).await?;
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["list"] => list_shares(&account, &cancel).await?,
        ["text", message] => println!("{}\npasscode: tiger lily 42", share_text(&account, message, &cancel).await?),
        // or the share host's API
        ["open", link] => open_link(&client, account.server_config().api, link.to_string(), &cancel).await?,
        _ => usage("Usage: share-links list | text <text> | open <link>"),
    }
    account.sign_out(false, cancel).await?;
    Ok(())
}
