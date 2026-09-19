use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use windows::core::HSTRING;
use windows::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};

use super::{parse_iso8601, PollError, HTTP_AGENT};
use crate::diagnose;
use crate::models::{UsageData, UsageSection};

const ANTIGRAVITY_CREDENTIAL_TARGET: &str = "gemini:antigravity";
const ANTIGRAVITY_ENDPOINTS: &[&str] = &[
    "https://daily-cloudcode-pa.googleapis.com",
    "https://daily-cloudcode-pa.sandbox.googleapis.com",
    "https://cloudcode-pa.googleapis.com",
];

#[derive(Deserialize)]
struct AntigravityAuthFile {
    token: AntigravityTokenData,
}

#[derive(Deserialize)]
struct AntigravityTokenData {
    access_token: String,
}

#[derive(Deserialize)]
struct AntigravityLoadResponse {
    #[serde(rename = "cloudaicompanionProject")]
    project: Option<String>,
}

#[derive(Deserialize)]
struct AntigravityModelsResponse {
    models: HashMap<String, AntigravityModelInfo>,
}

#[derive(Deserialize)]
struct AntigravityModelInfo {
    #[serde(rename = "quotaInfo")]
    quota_info: Option<AntigravityQuotaInfo>,
}

#[derive(Deserialize)]
struct AntigravityQuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AntigravityQuotaSummaryResponse {
    groups: Option<Vec<AntigravityQuotaSummaryGroup>>,
}

#[derive(Deserialize)]
struct AntigravityQuotaSummaryGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    buckets: Option<Vec<AntigravityQuotaSummaryBucket>>,
}

#[derive(Clone, Deserialize)]
struct AntigravityQuotaSummaryBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

pub(super) fn poll_antigravity() -> Result<UsageData, PollError> {
    let creds = match read_antigravity_credentials() {
        Some(creds) => creds,
        None => {
            diagnose::log("Antigravity usage poll failed: no Antigravity credentials found");
            return Err(PollError::NoCredentials);
        }
    };

    fetch_antigravity_usage(&creds.access_token)
}

pub(super) fn credential_watch_signature() -> String {
    match read_windows_generic_credential(ANTIGRAVITY_CREDENTIAL_TARGET) {
        Some(content) => format!(
            "{ANTIGRAVITY_CREDENTIAL_TARGET}|present|{}|{}",
            content.len(),
            crate::accounts::fingerprint(&content)
        ),
        None => format!("{ANTIGRAVITY_CREDENTIAL_TARGET}|missing"),
    }
}

fn fetch_antigravity_usage(token: &str) -> Result<UsageData, PollError> {
    let mut auth_error = false;
    let mut last_error = PollError::RequestFailed;

    for base_url in ANTIGRAVITY_ENDPOINTS {
        match fetch_antigravity_usage_from_endpoint(base_url, token) {
            Ok(data) => return Ok(data),
            Err(PollError::AuthRequired) => auth_error = true,
            Err(error) => last_error = error,
        }
    }

    if auth_error {
        Err(PollError::AuthRequired)
    } else {
        Err(last_error)
    }
}

fn fetch_antigravity_usage_from_endpoint(
    base_url: &str,
    token: &str,
) -> Result<UsageData, PollError> {
    let project = post_json::<AntigravityLoadResponse>(
        base_url,
        "loadCodeAssist",
        token,
        serde_json::json!({ "metadata": { "ideType": "ANTIGRAVITY" } }),
    )?
    .project
    .filter(|project| !project.is_empty());

    if let Some(project) = project.as_deref() {
        match post_json(
            base_url,
            "retrieveUserQuotaSummary",
            token,
            serde_json::json!({ "project": project }),
        )
        .and_then(|response| {
            antigravity_usage_from_summary(response).ok_or(PollError::RequestFailed)
        }) {
            Ok(data) => return Ok(data),
            Err(PollError::AuthRequired) => return Err(PollError::AuthRequired),
            Err(error) => diagnose::log(format!(
                "Antigravity retrieveUserQuotaSummary failed, falling back to model quota: {error:?}"
            )),
        }
    }

    let body = match project {
        Some(project) => serde_json::json!({ "project": project }),
        None => serde_json::json!({}),
    };
    let response: AntigravityModelsResponse =
        post_json(base_url, "fetchAvailableModels", token, body)?;
    let session =
        best_antigravity_section(response.models.into_iter().filter_map(|(model, info)| {
            let quota = info.quota_info?;
            if !is_antigravity_display_model(&model) {
                return None;
            }
            section_from_remaining(quota.remaining_fraction, quota.reset_time.as_deref())
        }))
        .ok_or(PollError::RequestFailed)?;

    Ok(UsageData {
        session,
        ..Default::default()
    })
}

/// POST one `v1internal` method and read its JSON reply.
fn post_json<T: DeserializeOwned>(
    base_url: &str,
    method: &str,
    token: &str,
    body: serde_json::Value,
) -> Result<T, PollError> {
    let mut response = match HTTP_AGENT
        .post(&format!("{base_url}/v1internal:{method}"))
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "antigravity")
        .send_json(&body)
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(code @ (401 | 403))) => {
            diagnose::log(format!(
                "Antigravity {method} returned auth error status {code}"
            ));
            return Err(PollError::AuthRequired);
        }
        Err(error) => {
            diagnose::log_error(&format!("Antigravity {method} request failed"), error);
            return Err(PollError::RequestFailed);
        }
    };
    response.body_mut().read_json().map_err(|error| {
        diagnose::log_error(
            &format!("unable to parse Antigravity {method} response"),
            error,
        );
        PollError::RequestFailed
    })
}

pub(super) fn section_from_remaining(
    remaining_fraction: Option<f64>,
    reset_time: Option<&str>,
) -> Option<UsageSection> {
    let remaining = remaining_fraction?.clamp(0.0, 1.0);
    Some(UsageSection {
        available: true,
        percentage: (1.0 - remaining) * 100.0,
        resets_at: parse_iso8601(reset_time),
    })
}

pub(super) fn antigravity_usage_from_summary(
    response: AntigravityQuotaSummaryResponse,
) -> Option<UsageData> {
    let mut fallback = None;

    for group in response.groups.unwrap_or_default() {
        let is_gemini = is_antigravity_gemini_summary_group(&group);
        let usage = antigravity_usage_from_summary_group(group);

        if is_gemini && usage.is_some() {
            return usage;
        }

        if fallback.is_none() {
            fallback = usage;
        }
    }

    fallback
}

fn antigravity_usage_from_summary_group(group: AntigravityQuotaSummaryGroup) -> Option<UsageData> {
    let mut data = UsageData::default();
    let mut has_quota = false;

    for bucket in group.buckets.unwrap_or_default() {
        let Some(section) =
            section_from_remaining(bucket.remaining_fraction, bucket.reset_time.as_deref())
        else {
            continue;
        };

        match bucket.window.as_deref() {
            Some(window) if window.eq_ignore_ascii_case("5h") => {
                data.session = section;
                has_quota = true;
            }
            Some(window) if window.eq_ignore_ascii_case("weekly") => {
                data.weekly = section;
                has_quota = true;
            }
            _ => {}
        }
    }

    has_quota.then_some(data)
}

fn is_antigravity_gemini_summary_group(group: &AntigravityQuotaSummaryGroup) -> bool {
    group
        .display_name
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().contains("gemini"))
        || group
            .description
            .as_deref()
            .is_some_and(|description| description.to_ascii_lowercase().contains("gemini"))
        || group.buckets.as_ref().is_some_and(|buckets| {
            buckets.iter().any(|bucket| {
                bucket
                    .bucket_id
                    .as_deref()
                    .is_some_and(|id| id.to_ascii_lowercase().starts_with("gemini-"))
                    || bucket
                        .display_name
                        .as_deref()
                        .is_some_and(|name| name.to_ascii_lowercase().contains("gemini"))
            })
        })
}

fn best_antigravity_section<I>(sections: I) -> Option<UsageSection>
where
    I: IntoIterator<Item = UsageSection>,
{
    sections.into_iter().max_by(|a, b| {
        a.percentage
            .partial_cmp(&b.percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.resets_at.cmp(&b.resets_at))
    })
}

fn is_antigravity_display_model(model: &str) -> bool {
    model.starts_with("gemini")
        || model.starts_with("claude")
        || model.starts_with("gpt")
        || model.starts_with("image")
        || model.starts_with("imagen")
}

fn read_antigravity_credentials() -> Option<AntigravityTokenData> {
    let content = read_windows_generic_credential(ANTIGRAVITY_CREDENTIAL_TARGET)?;
    let auth: AntigravityAuthFile = serde_json::from_str(&content).ok()?;
    (!auth.token.access_token.is_empty()).then_some(auth.token)
}

fn read_windows_generic_credential(target: &str) -> Option<String> {
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    let read = unsafe {
        CredReadW(
            &HSTRING::from(target),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
    };
    if read.is_err() || credential.is_null() {
        diagnose::log(format!(
            "unable to read Windows generic credential target {target}"
        ));
        return None;
    }

    unsafe {
        let blob = (*credential).CredentialBlob;
        let size = (*credential).CredentialBlobSize as usize;
        let text = if size == 0 || blob.is_null() {
            None
        } else {
            String::from_utf8(std::slice::from_raw_parts(blob, size).to_vec()).ok()
        };
        CredFree(credential.cast());
        text
    }
}
