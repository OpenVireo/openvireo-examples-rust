//! Vaults: listing and opening, reading and writing fields, editing entries, creating, sharing,
//! revoking and organisations.
//!
//! Guide: https://developer.openvireo.com/sdk/guides/vaults-and-sharing/
//!
//! Running it signs in, lists the vaults and the entries of the first one, and reads a field if you
//! pass an ov:// reference. Nothing is written unless you pass --write, which adds an "OpenVireo
//! example" entry to the first vault you can write to. The sharing and organisation functions are
//! here to read and copy; run() doesn't call them.
//!
//!     cargo run -- vaults-and-sharing ["ov://Vault/Entry/field"] [--write]
#![allow(dead_code)]

use ov_core::api::{
    Account, CancelToken, CreateMode, EntryInput, FieldInput, Grantee, MemberStatus, OrgRole, SyncOptions, Vault,
    VaultGrantRequest, VaultRole,
};

use crate::common::{make_client, prompt, sign_in, text, Result};

// Listing and opening
async fn list_and_open(account: &Account, cancel: &CancelToken) -> Result<Option<Vault>> {
    let vaults = account.vaults(cancel.clone()).await?; // VaultInfo: id, name, role (Read, Write, Manage), org_id
    for info in &vaults {
        println!("{} {:?} {}", info.name, info.role, info.id);
    }
    let Some(first) = vaults.first() else { return Ok(None) };

    let vault = account.open_vault(first.id.clone(), cancel.clone()).await?; // downloads and decrypts it
    println!("Entries in {} at {}", vault.name(), vault.etag().unwrap_or_default()); // the server version this copy is based on
    for e in vault.entries(false) {
        println!("  {}", e.title); // vault.entry(id) gives the full record
    }
    Ok(Some(vault))
}

// Reading and writing fields
async fn read_and_write(vault: &Vault, key: Vec<u8>, cancel: &CancelToken) -> Result {
    println!("{}", text(&vault.read_field("ov://Team/GitHub/password".into())?));

    // edits and syncs; create: true creates the entry if needed
    vault.write_field("ov://Team/GitHub/password".into(), b"new value".to_vec(), false, cancel.clone()).await?;
    vault.write_field("ov://Team/Apps/Stripe/api_key".into(), key, true, cancel.clone()).await?;
    Ok(())
}

// Editing the database directly
async fn edit_entries(vault: &Vault, cancel: &CancelToken) -> Result {
    let field = |name: &str, value: &str, protected: Option<bool>| FieldInput {
        name: name.into(),
        value: value.as_bytes().to_vec(),
        protected,
    };
    vault.add_entry(
        vault.root_group_id(),
        EntryInput {
            fields: vec![
                field("Title", "OpenVireo example", None),
                field("UserName", "app", None),
                field("Password", "s3cret", None),
            ],
            ..Default::default()
        },
    )?;
    vault.set_fields(
        format!("ov://{}/OpenVireo example", vault.name()),
        vec![field("UserName", "svc", None), field("token", "...", Some(true))],
        CreateMode::CreateIfMissing,
    )?;

    let result = vault.sync(SyncOptions { local_changed: true, accept_server_state: false }, cancel.clone()).await?;
    println!("{:?}", result.outcome); // Uploaded, Pulled (only downloaded), Created or Unchanged
    println!("{:?}", result.conflicts); // entries both sides changed
    println!("{:?}", result.changes); // what the merge did, entry by entry
    Ok(())
}

// Creating vaults
async fn create_vaults(account: &Account, org_id: String, cancel: &CancelToken) -> Result {
    account.create_vault("Personal".into(), None, cancel.clone()).await?;
    account.create_vault("Team Infra".into(), Some(org_id), cancel.clone()).await?; // admins get manage
    Ok(())
}

// Sharing
async fn share_by_email(account: &Account, vault_id: String, cancel: &CancelToken) -> Result {
    let grant = account
        .share_vault(vault_id.clone(), Grantee::Email("bob@example.com".into()), VaultRole::Write, cancel.clone())
        .await?;
    confirm_with_bob(&grant.fingerprint)?; // compare it with what Bob's app shows, out of band

    account.change_role(vault_id, grant.principal, VaultRole::Read, cancel.clone()).await?;
    Ok(())
}

async fn share_with_verified_keys(account: &Account, vault_id: String, cancel: &CancelToken) -> Result {
    let bob = account.look_up("bob@example.com".into(), cancel.clone()).await?; // keys verified
    confirm_with_bob(&bob.public_keys.fingerprint)?;
    account
        .share_vault_with_keys(
            vault_id,
            Grantee::Principal(bob.principal),
            VaultRole::Write,
            bob.public_keys,
            cancel.clone(),
        )
        .await?;
    Ok(())
}

fn confirm_with_bob(fingerprint: &str) -> Result {
    if prompt(&format!("Does Bob's app show {fingerprint}? [y/N] ")).to_lowercase() != "y" {
        return Err("not confirmed".into());
    }
    Ok(())
}

// Revoking and rotating
async fn revoke_and_rotate(account: &Account, vault_id: String, bob_principal: String, cancel: &CancelToken) -> Result {
    account.revoke(vault_id.clone(), vec![bob_principal], cancel.clone()).await?; // and rotates the key
    account.rotate_vault_key(vault_id, vec![], cancel.clone()).await?; // a new key only
    Ok(())
}

// Organisations
async fn organisations(account: &Account, vault_id: String, cancel: &CancelToken) -> Result {
    let org = account.create_org("Acme".into(), cancel.clone()).await?;
    account.invite(org.id.clone(), "carol@example.com".into(), OrgRole::Member, cancel.clone()).await?;
    // Carol gets an email, and her app calls account.accept_invitation(token, cancel)

    let members = account.org_members(org.id.clone(), cancel.clone()).await?;
    let Some(carol) =
        members.into_iter().find(|m| m.email == "carol@example.com" && m.status == MemberStatus::Accepted)
    else {
        return Ok(());
    };
    let grants = vec![VaultGrantRequest { vault_id, role: VaultRole::Read }];
    account.confirm_member(org.id.clone(), carol.clone(), grants, cancel.clone()).await?;

    account.set_member_role(org.id.clone(), carol.clone(), OrgRole::Admin, cancel.clone()).await?; // manage everywhere
    let report = account.remove_member(org.id, carol.user_id, cancel.clone()).await?; // rotates her vaults
    println!("rotated: {:?} failed: {:?}", report.rotated, report.failed);
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result {
    let cancel = CancelToken::new();
    let account = sign_in(&make_client("OpenVireo examples", false), &cancel).await?;
    let result = async {
        let vault = list_and_open(&account, &cancel).await?;
        if let (Some(vault), Some(reference)) = (&vault, args.iter().find(|a| a.starts_with("ov://"))) {
            println!("{reference} = {}", text(&vault.read_field(reference.clone())?));
        }
        if args.iter().any(|a| a == "--write") {
            let vaults = account.vaults(cancel.clone()).await?;
            if let Some(info) = vaults.iter().find(|v| v.role != VaultRole::Read) {
                edit_entries(&account.open_vault(info.id.clone(), cancel.clone()).await?, &cancel).await?;
            }
        }
        Result::Ok(())
    }
    .await;
    account.sign_out(false, cancel).await?;
    result
}
