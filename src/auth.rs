//! The optional online account: a Minecraft session handed to the game.
//!
//! Horizon reads an account from `--session-stdin` (or `--session-file`) as
//! `{"name", "uuid", "access_token", "user_type": "msa", "xuid"}` — the shape
//! its Launch API documents. This file is the session itself: its shape, the
//! JSON a session file or a paste brings in, the check against Mojang's own
//! services, and the refresh token that lets [`crate::microsoft`] mint a new one
//! without asking the player again.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::valid_player_name;
use crate::Result;

const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const ENTITLEMENTS: &str = "https://api.minecraftservices.com/entitlements/mcstore";

/// A Minecraft access token and who it belongs to.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    #[serde(default = "default_user_type")]
    pub user_type: String,
    #[serde(default)]
    pub xuid: Option<String>,
    /// When Sunrise last checked this session with Mojang.
    #[serde(default)]
    pub checked_at: u64,
    /// What renews the session without asking the player again. Only a sign-in
    /// through [`crate::microsoft`] has one; an imported session does not.
    /// Sessions saved by older builds may also name a Microsoft application id
    /// here; that field is ignored now, so nothing has to be migrated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// When the Minecraft token itself was minted: Minecraft tokens last a day.
    #[serde(default)]
    pub issued_at: u64,
}

fn default_user_type() -> String {
    "msa".to_string()
}

/// Only what the game needs, in the Launch API's field names.
#[derive(Serialize)]
struct Handover<'a> {
    name: &'a str,
    uuid: &'a str,
    access_token: &'a str,
    user_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    xuid: Option<&'a str>,
}

impl Session {
    pub fn handover_json(&self) -> String {
        let xuid = self.xuid.as_deref().filter(|x| !x.is_empty());
        serde_json::to_string(&Handover {
            name: &self.name,
            uuid: &self.uuid,
            access_token: &self.access_token,
            user_type: if self.user_type.is_empty() {
                "msa"
            } else {
                &self.user_type
            },
            xuid,
        })
        .unwrap_or_default()
    }

    /// Minecraft tokens last 24 hours. Sunrise cannot renew one without its own
    /// approved application, so past 20 hours it says so and still tries.
    pub fn looks_stale(&self) -> bool {
        self.checked_at > 0 && crate::now_secs() > self.checked_at.saturating_add(20 * 3600)
    }

    pub fn short_uuid(&self) -> String {
        self.uuid.get(..8).unwrap_or(&self.uuid).to_string()
    }

    /// Whether Sunrise can get a fresh token without the player's help.
    pub fn renewable(&self) -> bool {
        self.refresh_token
            .as_deref()
            .map(|token| !token.trim().is_empty())
            .unwrap_or(false)
    }

    /// A Minecraft token lasts 24 hours; past 20 Sunrise renews it first.
    pub fn needs_renewal(&self) -> bool {
        self.renewable()
            && self.issued_at > 0
            && crate::now_secs() > self.issued_at.saturating_add(20 * 3600)
    }
}

fn clean_uuid(raw: &str) -> Option<String> {
    let compact: String = raw
        .chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if compact.len() == 32 && compact.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(compact)
    } else {
        None
    }
}

/// Read a session from JSON. Accepts the Launch API fields, and also the nested
/// `{"minecraftProfile": {...}}` shape some launchers write.
pub fn parse_session(text: &str) -> Result<Session> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("that is not JSON: {e}"))?;

    let source = if value.get("access_token").is_some() {
        &value
    } else if let Some(profile) = value.get("minecraftProfile") {
        profile
    } else {
        &value
    };

    let pick = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .find_map(|key| source.get(*key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    let access_token = pick(&["access_token", "accessToken"]).ok_or_else(|| {
        "no access_token in that JSON: Sunrise needs Horizon's session shape".to_string()
    })?;
    let uuid =
        pick(&["uuid", "remoteId", "id"]).ok_or_else(|| "no uuid in that JSON".to_string())?;
    let uuid = clean_uuid(&uuid).ok_or_else(|| format!("'{uuid}' is not a Minecraft uuid"))?;
    // The profile name is optional: Mojang's answer replaces it below.
    let name = pick(&["name", "username", "playerName"]).unwrap_or_default();
    let xuid = pick(&["xuid"]);
    let user_type = pick(&["user_type", "userType"]).unwrap_or_else(default_user_type);

    Ok(Session {
        name,
        uuid,
        access_token,
        user_type,
        xuid,
        checked_at: 0,
        ..Default::default()
    })
}

#[derive(Clone, Debug)]
pub struct Checked {
    pub session: Session,
    /// Things that are true but not fatal, such as an unconfirmed ownership.
    pub warning: Option<String>,
}

fn http() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("SunriseLauncher/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("could not start the HTTP client: {e}"))
}

fn json(body: &str) -> Option<serde_json::Value> {
    serde_json::from_str(body).ok()
}

/// Ask Mojang who this session is, whether it still works, and whether the
/// account owns Java Edition. A working session is required; ownership is
/// reported, because an unconfirmed answer should not stop an offline launch.
pub fn check(session: &Session) -> Result<Checked> {
    if session.access_token.trim().is_empty() {
        return Err("that session has no access token".to_string());
    }
    let client = http()?;
    let auth = format!("Bearer {}", session.access_token.trim());

    let response = client
        .get(PROFILE)
        .header("Authorization", &auth)
        .send()
        .map_err(|e| format!("could not reach Mojang to check the session: {e}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();

    match status.as_u16() {
        200 => {}
        401 | 403 => {
            return Err(
                "Mojang refused this session, so it has expired or was signed out. Import a fresh one."
                    .to_string(),
            )
        }
        404 => {
            return Err(
                "the account behind this session owns Minecraft but has no profile yet; create one on minecraft.net first."
                    .to_string(),
            )
        }
        other => {
            return Err(format!(
                "Mojang answered {other} while checking the session: {}",
                body.chars().take(160).collect::<String>()
            ))
        }
    }

    let profile =
        json(&body).ok_or_else(|| "Mojang's profile answer was not readable".to_string())?;
    let name = profile
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&session.name)
        .to_string();
    let uuid = profile
        .get("id")
        .and_then(serde_json::Value::as_str)
        .and_then(clean_uuid)
        .unwrap_or_else(|| session.uuid.clone());

    if !valid_player_name(&name) {
        return Err(format!(
            "Mojang says the profile name is '{name}', which is not a valid name"
        ));
    }

    let mut warning = None;
    match client
        .get(ENTITLEMENTS)
        .header("Authorization", &auth)
        .send()
    {
        Ok(response) if response.status().is_success() => {
            let body = response.text().unwrap_or_default();
            let owned = json(&body)
                .and_then(|v| {
                    v.get("items").and_then(|items| {
                        items.as_array().map(|list| {
                            list.iter().any(|item| {
                                matches!(
                                    item.get("name").and_then(serde_json::Value::as_str),
                                    Some("product_minecraft") | Some("game_minecraft")
                                )
                            })
                        })
                    })
                })
                .unwrap_or(false);
            if !owned {
                return Err(
                    "this account does not own Minecraft: Java Edition, so online-mode servers will refuse it."
                        .to_string(),
                );
            }
        }
        Ok(_) | Err(_) => {
            warning = Some("could not confirm that this account owns Java Edition".to_string());
        }
    }

    Ok(Checked {
        session: Session {
            name,
            uuid,
            access_token: session.access_token.trim().to_string(),
            user_type: if session.user_type.is_empty() {
                default_user_type()
            } else {
                session.user_type.clone()
            },
            xuid: session.xuid.clone(),
            checked_at: crate::now_secs(),
            refresh_token: session.refresh_token.clone(),
            issued_at: if session.issued_at > 0 {
                session.issued_at
            } else {
                crate::now_secs()
            },
        },
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_api_shape_is_read_and_written() {
        let text = r#"{"name":"Alex","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","access_token":"tok","user_type":"msa","xuid":"123"}"#;
        let session = parse_session(text).unwrap();
        assert_eq!(session.name, "Alex");
        assert_eq!(session.uuid, "069a79f444e94726a5befca90e38aaf5");
        let out: serde_json::Value = serde_json::from_str(&session.handover_json()).unwrap();
        assert_eq!(out["name"], "Alex");
        assert_eq!(out["uuid"], "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(out["access_token"], "tok");
        assert_eq!(out["user_type"], "msa");
        assert_eq!(out["xuid"], "123");
        // The token Sunrise keeps for itself must not leak into the handover.
        assert!(out.get("checked_at").is_none());
    }

    #[test]
    fn a_nested_launcher_profile_is_read_too() {
        let text = r#"{"minecraftProfile":{"id":"069a79f444e94726a5befca90e38aaf5","name":"Alex","accessToken":"tok"}}"#;
        let session = parse_session(text).unwrap();
        assert_eq!(session.name, "Alex");
        assert_eq!(session.access_token, "tok");
    }

    #[test]
    fn bad_sessions_are_refused_with_a_reason() {
        assert!(parse_session("{}").unwrap_err().contains("access_token"));
        assert!(parse_session(r#"{"uuid":"abc","access_token":"t"}"#)
            .unwrap_err()
            .contains("uuid"));
        assert!(parse_session("not json").unwrap_err().contains("JSON"));
    }

    #[test]
    fn freshness_follows_the_twenty_hour_mark() {
        let mut session = Session {
            checked_at: crate::now_secs(),
            ..Default::default()
        };
        assert!(!session.looks_stale());
        session.checked_at = crate::now_secs().saturating_sub(21 * 3600);
        assert!(session.looks_stale());
    }
}
