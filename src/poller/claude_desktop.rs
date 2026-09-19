//! Reads the OAuth token that the Claude desktop app keeps for its bundled
//! Claude Code build.
//!
//! Machines that only ever ran Claude Code through the desktop app have no
//! `~/.claude/.credentials.json`, because that file is written by the
//! standalone CLI login flow. The desktop app is an Electron application and
//! stores its token cache with Chromium's OSCrypt scheme instead: an
//! AES-256-GCM key sits DPAPI-wrapped in `Local State`, and each encrypted
//! value is `"v10" || nonce || ciphertext || tag`.
//!
//! Everything here is read-only, runs as the signed-in user, and degrades to
//! `None` whenever the layout is not what we expect.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    BCryptDecrypt, BCryptDestroyKey, BCryptGenerateSymmetricKey, CryptUnprotectData,
    BCRYPT_AES_GCM_ALG_HANDLE, BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO,
    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION, BCRYPT_FLAGS, BCRYPT_KEY_HANDLE,
    CRYPT_INTEGER_BLOB,
};
use windows::Win32::UI::Shell::FOLDERID_RoamingAppData;

use crate::diagnose;

/// Newest layout first. The desktop app migrated its cache to
/// `oauth:tokenCacheV2` and leaves the older `oauth:tokenCache` key in place,
/// so both are tried and the first that yields a usable token wins.
const TOKEN_CACHE_KEYS: &[&str] = &["oauth:tokenCacheV2", "oauth:tokenCache"];
const DPAPI_KEY_PREFIX: &[u8] = b"DPAPI";
const OS_CRYPT_PREFIX: &[u8] = b"v10";
const GCM_NONCE_LEN: usize = 12;
const GCM_TAG_LEN: usize = 16;
/// Desktop entries are keyed `"<install>:<user>:<base url>:<scopes>"`; the
/// inference scope marks the token the usage endpoint accepts.
const INFERENCE_SCOPE: &str = "user:inference";

pub(super) struct DesktopToken {
    pub(super) access_token: String,
    pub(super) expires_at: Option<i64>,
}

pub(super) fn config_path() -> Option<PathBuf> {
    Some(
        crate::accounts::known_folder(FOLDERID_RoamingAppData)?
            .join("Claude")
            .join("config.json"),
    )
}

fn local_state_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("Local State")
}

pub(super) fn read_token(config_path: &Path) -> Option<DesktopToken> {
    let config = match std::fs::read_to_string(config_path) {
        Ok(config) => config,
        Err(error) => {
            diagnose::log_error(
                &format!(
                    "unable to read Claude desktop config at {}",
                    config_path.display()
                ),
                error,
            );
            return None;
        }
    };

    let caches = token_cache_values(&config);
    if caches.is_empty() {
        diagnose::log("Claude desktop config held no OAuth token cache");
        return None;
    }
    let key = os_crypt_key(&local_state_path(config_path))?;

    // The app writes both caches and does not always refresh both, so an
    // expired V2 entry must not mask a live legacy one: take the best across
    // all caches rather than the first that parses. Live before expired, then
    // the later expiry, is simply the later expiry; a token without one is
    // taken at its word. Ties keep the earlier cache.
    let expiry = |token: &DesktopToken| token.expires_at.unwrap_or(i64::MAX);
    let mut best: Option<DesktopToken> = None;
    for (name, cache) in &caches {
        let Some(plaintext) = decrypt_os_crypt_value(cache, &key) else {
            diagnose::log(format!("unable to decrypt Claude desktop {name}"));
            continue;
        };
        let Ok(plaintext) = String::from_utf8(plaintext) else {
            diagnose::log(format!("Claude desktop {name} was not valid UTF-8"));
            continue;
        };
        let Some(token) = select_token(&plaintext) else {
            diagnose::log(format!(
                "Claude desktop {name} held no usable inference token"
            ));
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|current| expiry(&token) > expiry(current))
        {
            best = Some(token);
        }
    }

    best
}

/// Signature over the encrypted cache rather than the file's mtime: the
/// desktop app rewrites `config.json` for unrelated state such as window
/// placement, and that must not read as a credential change.
pub(super) fn watch_signature(config_path: &Path) -> String {
    let key = format!("desktop:{}", config_path.display());
    let caches = std::fs::read_to_string(config_path)
        .ok()
        .map(|config| token_cache_values(&config))
        .unwrap_or_default();
    if caches.is_empty() {
        return format!("{key}|missing");
    }

    let mut signature = format!("{key}|present");
    for (name, cache) in caches {
        signature.push_str(&format!("|{name}:{}", crate::accounts::fingerprint(&cache)));
    }
    signature
}

/// Every token cache the config carries, newest layout first.
fn token_cache_values(config: &str) -> Vec<(&'static str, String)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(config) else {
        return Vec::new();
    };
    TOKEN_CACHE_KEYS
        .iter()
        .filter_map(|key| {
            let value = json.get(*key)?.as_str()?;
            (!value.is_empty()).then(|| (*key, value.to_string()))
        })
        .collect()
}

/// Picks the freshest entry that carries the inference scope, falling back to
/// the freshest entry of any scope so a future key layout still resolves.
fn select_token(plaintext: &str) -> Option<DesktopToken> {
    let json: serde_json::Value = serde_json::from_str(plaintext).ok()?;
    let entries = json.as_object()?;

    let mut best: Option<(bool, i64, DesktopToken)> = None;
    for (key, entry) in entries {
        let Some(access_token) = entry.get("token").and_then(|value| value.as_str()) else {
            continue;
        };
        if access_token.is_empty() {
            continue;
        }
        let expires_at = entry.get("expiresAt").and_then(|value| value.as_i64());
        let rank = (
            key.contains(INFERENCE_SCOPE),
            expires_at.unwrap_or(i64::MIN),
        );
        if best
            .as_ref()
            .is_some_and(|(scoped, expiry, _)| (*scoped, *expiry) >= rank)
        {
            continue;
        }
        best = Some((
            rank.0,
            rank.1,
            DesktopToken {
                access_token: access_token.to_string(),
                expires_at,
            },
        ));
    }

    best.map(|(_, _, token)| token)
}

fn os_crypt_key(local_state_path: &Path) -> Option<Vec<u8>> {
    let local_state = std::fs::read_to_string(local_state_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&local_state).ok()?;
    let encoded = json.get("os_crypt")?.get("encrypted_key")?.as_str()?;
    let wrapped = base64_decode(encoded)?;
    let wrapped = wrapped.strip_prefix(DPAPI_KEY_PREFIX)?;
    dpapi_unprotect(wrapped)
}

fn decrypt_os_crypt_value(value: &str, key: &[u8]) -> Option<Vec<u8>> {
    let blob = base64_decode(value)?;
    let body = blob.strip_prefix(OS_CRYPT_PREFIX)?;
    if body.len() < GCM_NONCE_LEN + GCM_TAG_LEN {
        return None;
    }
    let (nonce, rest) = body.split_at(GCM_NONCE_LEN);
    let (ciphertext, tag) = rest.split_at(rest.len() - GCM_TAG_LEN);
    aes_gcm_decrypt(key, nonce, ciphertext, tag)
}

/// Chromium writes standard, padded base64.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    super::base64_decode(input.trim_end_matches('='), false)
}

fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).ok()?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let unwrapped = unsafe { CryptUnprotectData(&input, None, None, None, None, 0, &mut output) };
    if unwrapped.is_err() || output.pbData.is_null() {
        diagnose::log("unable to unwrap the Claude desktop OSCrypt key with DPAPI");
        return None;
    }

    unsafe {
        let key = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(output.pbData.cast())));
        Some(key)
    }
}

fn aes_gcm_decrypt(key: &[u8], nonce: &[u8], ciphertext: &[u8], tag: &[u8]) -> Option<Vec<u8>> {
    // The AES-GCM pseudo-handle needs no provider to open, configure or close,
    // and CNG allocates the key object itself.
    let mut key_handle = BCRYPT_KEY_HANDLE::default();
    let status = unsafe {
        BCryptGenerateSymmetricKey(BCRYPT_AES_GCM_ALG_HANDLE, &mut key_handle, None, key, 0)
    };
    if status.0 != 0 {
        diagnose::log("unable to import the Claude desktop OSCrypt key");
        return None;
    }

    // Decryption only reads the nonce and the tag.
    let mode_info = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
        pbNonce: nonce.as_ptr().cast_mut(),
        cbNonce: nonce.len() as u32,
        pbTag: tag.as_ptr().cast_mut(),
        cbTag: tag.len() as u32,
        ..Default::default()
    };
    let mut plaintext = vec![0u8; ciphertext.len()];
    let mut written = 0u32;
    let status = unsafe {
        BCryptDecrypt(
            key_handle,
            Some(ciphertext),
            Some(std::ptr::from_ref(&mode_info).cast()),
            None,
            Some(&mut plaintext),
            &mut written,
            BCRYPT_FLAGS(0),
        )
    };
    unsafe {
        let _ = BCryptDestroyKey(key_handle);
    }

    if status.0 != 0 {
        diagnose::log("Claude desktop token cache failed AES-GCM authentication");
        return None;
    }

    plaintext.truncate(written as usize);
    Some(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_the_freshest_inference_scoped_token() {
        let plaintext = r#"{
            "install:user:https://api.anthropic.com:user:profile": {
                "token": "profile-only",
                "expiresAt": 9000000000000
            },
            "install:user:https://api.anthropic.com:user:inference user:profile": {
                "token": "current",
                "expiresAt": 1818644595762
            },
            "install:old:https://api.anthropic.com:user:inference": {
                "token": "stale",
                "expiresAt": 1518644595762
            }
        }"#;

        let token = select_token(plaintext).expect("an inference token should be selected");
        assert_eq!(token.access_token, "current");
        assert_eq!(token.expires_at, Some(1818644595762));
    }

    #[test]
    fn ignores_entries_without_a_usable_token() {
        assert!(select_token(r#"{"install:user:scope": {"expiresAt": 1}}"#).is_none());
        assert!(select_token(r#"{"install:user:scope": {"token": ""}}"#).is_none());
        assert!(select_token("not json").is_none());
    }

    #[test]
    fn reads_the_token_cache_out_of_a_desktop_config() {
        let config = r#"{"locale": "en-US", "oauth:tokenCache": "djEwYWJj"}"#;
        assert_eq!(
            token_cache_values(config),
            vec![("oauth:tokenCache", "djEwYWJj".to_string())]
        );
        assert!(token_cache_values(r#"{"locale": "en-US"}"#).is_empty());
        assert!(token_cache_values("not json").is_empty());
    }

    #[test]
    fn prefers_the_v2_cache_but_keeps_the_legacy_one_as_a_fallback() {
        let config = r#"{"oauth:tokenCache": "djEwb2xk", "oauth:tokenCacheV2": "djEwbmV3"}"#;
        assert_eq!(
            token_cache_values(config),
            vec![
                ("oauth:tokenCacheV2", "djEwbmV3".to_string()),
                ("oauth:tokenCache", "djEwb2xk".to_string()),
            ]
        );
    }

    #[test]
    fn ignores_emptied_token_caches() {
        // The desktop app leaves the key in place with an empty value after a
        // migration; that must not mask a populated cache under the other key.
        let config = r#"{"oauth:tokenCacheV2": "", "oauth:tokenCache": "djEwb2xk"}"#;
        assert_eq!(
            token_cache_values(config),
            vec![("oauth:tokenCache", "djEwb2xk".to_string())]
        );
    }

    #[test]
    fn rejects_blobs_that_are_not_os_crypt_v10() {
        // Valid base64, but the version prefix is not "v10".
        assert!(decrypt_os_crypt_value("bm90LXYxMC1kYXRh", &[0u8; 32]).is_none());
        // Right prefix, too short to hold a nonce and a tag.
        assert!(decrypt_os_crypt_value("djEwc2hvcnQ", &[0u8; 32]).is_none());
        assert!(decrypt_os_crypt_value("!!!", &[0u8; 32]).is_none());
    }

    #[test]
    fn decodes_standard_base64_with_and_without_padding() {
        assert_eq!(base64_decode("djEw").unwrap(), b"v10");
        assert_eq!(base64_decode("YWJjZA==").unwrap(), b"abcd");
        assert!(base64_decode("a").is_none());
        assert!(base64_decode("a-b_").is_none());
    }

    #[test]
    fn decrypts_an_os_crypt_value_with_aes_gcm() {
        // Sealed by an independent AES-256-GCM implementation: key 0..32,
        // nonce 100..112, no associated data.
        const BLOB: &str = "djEwZGVmZ2hpamtsbW5vMzm3CAqdN/JSWCqbvxdQiDGndDDiApUX1bTCK56BnzO2nS+rKXy5Pt+AExKcLL7ifTLgk1U4JArW08mun+jX73vNryzsegM0s1nZdySzBNFSSmJJjt5TfVZg97h0YXPs2gfszg==";
        let key: Vec<u8> = (0..32).collect();
        let plaintext = decrypt_os_crypt_value(BLOB, &key).expect("the fixture should decrypt");
        let token = select_token(std::str::from_utf8(&plaintext).unwrap()).unwrap();
        assert_eq!(token.access_token, "sk-ant-fixture");
        assert_eq!(token.expires_at, Some(1818644595762));
        // A wrong key fails authentication instead of yielding garbage.
        assert!(decrypt_os_crypt_value(BLOB, &[0u8; 32]).is_none());
    }

    #[test]
    fn dpapi_unwraps_what_this_user_wrapped() {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

        let secret = b"os-crypt key fixture";
        let input = CRYPT_INTEGER_BLOB {
            cbData: secret.len() as u32,
            pbData: secret.as_ptr() as *mut u8,
        };
        let mut wrapped = CRYPT_INTEGER_BLOB::default();
        unsafe { CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut wrapped) }
            .unwrap();
        let bytes =
            unsafe { std::slice::from_raw_parts(wrapped.pbData, wrapped.cbData as usize) }.to_vec();
        unsafe { LocalFree(Some(HLOCAL(wrapped.pbData.cast()))) };

        assert_eq!(dpapi_unprotect(&bytes).as_deref(), Some(&secret[..]));
        assert!(dpapi_unprotect(b"not a DPAPI blob").is_none());
    }

    /// Ignored by default: this one proves the real DPAPI + AES-GCM path
    /// against whatever the Claude desktop app has on the current machine.
    /// Run it with `cargo test -- --ignored` while signed in to the app.
    #[test]
    #[ignore = "requires a signed-in Claude desktop app on this machine"]
    fn reads_a_token_from_the_installed_desktop_app() {
        let path = config_path().expect("a roaming config directory");
        let token = read_token(&path).expect("the desktop app should expose a token");
        assert!(token.access_token.starts_with("sk-ant-"));
        assert!(token.expires_at.unwrap_or_default() > 0);
    }

    #[test]
    fn watch_signature_reports_missing_config() {
        let signature = watch_signature(Path::new("C:/nonexistent/Claude/config.json"));
        assert!(signature.ends_with("|missing"), "{signature}");
    }
}
