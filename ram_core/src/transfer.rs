//! Portable account export / import for backup and transfer.
//!
//! The on-disk store ([`crate::crypto`]) is encrypted with a key that is bound
//! to this machine (Windows Credential Manager) or to a master password the
//! user types at startup. Either way the file alone is useless elsewhere, so it
//! cannot serve as a backup you can move between PCs.
//!
//! Export produces a plaintext JSON with every cookie decrypted. That is the
//! whole point of the feature — the export must be readable on a machine that
//! has never seen this store — which means **an export file is as sensitive as
//! the accounts themselves**. Anyone who gets the file gets the sessions. The
//! UI must say so and the user should treat it like a password manager vault
//! backup.
//!
//! Import is the reverse: it reads the JSON, re-encrypts every cookie under the
//! caller's live [`StoreSession`], and hands back a list of accounts ready to
//! merge into the store. Nothing is written by this module; the caller owns
//! when and whether the store gets saved.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::crypto::StoreSession;
use crate::error::CoreError;
use crate::models::Account;

/// Version of the export format. Bump when fields are added so an importer from
/// a newer RM can say "update RM" instead of silently dropping data.
const EXPORT_VERSION: u32 = 1;

/// One account as written to an export file.
///
/// `cookie` is the plaintext `.ROBLOSECURITY`. Everything else is copied from
/// the live account so a transfer round-trips the user's organization (alias,
/// notes, group) as well as the login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedAccount {
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub cookie: String,
}

/// Top-level export file. `format_version` lets a future release refuse files
/// it cannot read rather than guessing at them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    pub format_version: u32,
    pub accounts: Vec<ExportedAccount>,
}

impl ExportFile {
    fn new(accounts: Vec<ExportedAccount>) -> Self {
        Self {
            format_version: EXPORT_VERSION,
            accounts,
        }
    }
}

impl From<&ExportedAccount> for Account {
    fn from(e: &ExportedAccount) -> Self {
        let mut a = Account::new(e.user_id, e.username.clone(), e.display_name.clone());
        a.alias = e.alias.clone();
        a.notes = e.notes.clone();
        a.group = e.group.clone();
        a
    }
}

/// Decrypt every cookie in `store` and write a portable JSON export to `path`.
///
/// Returns the number of accounts written. The write is atomic
/// ([`crate::storage::atomic_write`]), so a crash mid-export never leaves a
/// half-written file that the user might mistake for a complete backup.
///
/// Accounts whose cookie cannot be decrypted are exported with an empty cookie
/// rather than failing the whole export: one unreadable session (e.g. stored
/// under a key that was retired) must not strand the other several dozen. The
/// empty cookie reads back as "no login stored" on import.
pub fn export_store(
    store: &crate::models::AccountStore,
    session: &StoreSession,
    path: &Path,
) -> Result<usize, CoreError> {
    let mut out = Vec::with_capacity(store.accounts.len());
    for account in &store.accounts {
        let cookie = match &account.encrypted_cookie {
            Some(enc) => match crate::crypto::decrypt_cookie(enc, session) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Export: could not decrypt cookie for user {} ({e}); exporting without it",
                        account.user_id
                    );
                    String::new()
                }
            },
            None => String::new(),
        };
        out.push(ExportedAccount {
            user_id: account.user_id,
            username: account.username.clone(),
            display_name: account.display_name.clone(),
            alias: account.alias.clone(),
            notes: account.notes.clone(),
            group: account.group.clone(),
            cookie,
        });
    }

    let json = serde_json::to_string_pretty(&ExportFile::new(out))?;
    crate::storage::atomic_write(path, json.as_bytes())?;
    Ok(store.accounts.len())
}

/// Read an export file and re-encrypt every cookie under `session`.
///
/// Returns the accounts with their cookies encrypted for this store. Accounts
/// whose export entry has an empty cookie come back with no cookie stored.
/// The caller decides how to merge them (skip duplicates, count them, save).
pub fn import_store(path: &Path, session: &StoreSession) -> Result<Vec<Account>, CoreError> {
    let text = std::fs::read_to_string(path)?;
    let file: ExportFile = serde_json::from_str(&text)?;

    if file.format_version != EXPORT_VERSION {
        return Err(CoreError::Crypto(format!(
            "this export was written by an incompatible version of RM \
             (format {}; this build reads {EXPORT_VERSION})",
            file.format_version
        )));
    }

    let mut accounts = Vec::with_capacity(file.accounts.len());
    for exported in &file.accounts {
        let mut account = Account::from(exported);
        if !exported.cookie.is_empty() {
            account.encrypted_cookie =
                Some(crate::crypto::encrypt_cookie(&exported.cookie, session)?);
        }
        accounts.push(account);
    }
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;
    use crate::models::{AccountStore, Presence};
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ram_transfer_{}_{name}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("export.json")
    }

    fn cleanup(p: &Path) {
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    fn session() -> StoreSession {
        crypto::create_password_session("test-pw").unwrap()
    }

    fn store_with_cookies(s: &StoreSession) -> AccountStore {
        let mut store = AccountStore::default();
        for i in 0..3u64 {
            let mut a = Account::new(i, format!("user{i}"), format!("User {i}"));
            a.alias = format!("alias{i}");
            a.notes = format!("note{i}");
            a.group = if i % 2 == 0 { "GroupA".into() } else { "".into() };
            a.encrypted_cookie = Some(crypto::encrypt_cookie(&format!("COOKIE_{i}"), s).unwrap());
            a.last_presence = Presence {
                user_presence_type: (i % 4) as u8,
                ..Default::default()
            };
            store.accounts.push(a);
        }
        store
    }

    #[test]
    fn export_then_import_round_trips_cookies_and_metadata() {
        let p = scratch("roundtrip");
        let s = session();
        let store = store_with_cookies(&s);

        let written = export_store(&store, &s, &p).unwrap();
        assert_eq!(written, 3);

        // A foreign session (as if importing on another PC) must still read it:
        // the export is plaintext, not bound to any device.
        let foreign = crypto::create_password_session("other-pw").unwrap();
        let imported = import_store(&p, &foreign).unwrap();
        assert_eq!(imported.len(), 3);

        for (i, a) in imported.iter().enumerate() {
            assert_eq!(a.user_id, i as u64);
            assert_eq!(a.alias, format!("alias{i}"));
            assert_eq!(a.notes, format!("note{i}"));
            assert_eq!(a.group, if i % 2 == 0 { "GroupA" } else { "" });
            // Re-encrypted under the foreign session and readable back.
            let plain = crypto::decrypt_cookie(
                a.encrypted_cookie.as_ref().unwrap(),
                &foreign,
            )
            .unwrap();
            assert_eq!(plain, format!("COOKIE_{i}"));
            // Presence is runtime state, not part of a transfer.
            assert_eq!(a.last_presence.user_presence_type, 0);
        }
        cleanup(&p);
    }

    #[test]
    fn export_skips_an_undecryptable_cookie_instead_of_failing() {
        let p = scratch("skipbad");
        let s = session();
        let other = crypto::create_password_session("different-key").unwrap();

        let mut store = store_with_cookies(&s);
        // This cookie is encrypted under a key the export session cannot read.
        store.accounts[1].encrypted_cookie =
            Some(crypto::encrypt_cookie("ORPHAN", &other).unwrap());

        let written = export_store(&store, &s, &p).unwrap();
        assert_eq!(written, 3, "one bad cookie must not fail the export");

        let imported = import_store(&p, &s).unwrap();
        assert_eq!(imported[0].encrypted_cookie.is_some(), true);
        assert_eq!(imported[1].encrypted_cookie, None, "bad cookie exported empty");
        assert_eq!(imported[2].encrypted_cookie.is_some(), true);
        cleanup(&p);
    }

    #[test]
    fn a_future_format_version_is_rejected() {
        let p = scratch("futurever");
        let s = session();
        export_store(&store_with_cookies(&s), &s, &p).unwrap();

        // Hand-edit the version to something this build does not know.
        let mut text = std::fs::read_to_string(&p).unwrap();
        text = text.replace("\"format_version\": 1", "\"format_version\": 99");
        std::fs::write(&p, &text).unwrap();

        let err = import_store(&p, &s).unwrap_err().to_string();
        assert!(err.contains("incompatible version"), "unhelpful error: {err}");
        cleanup(&p);
    }
}
