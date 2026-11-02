//! Signing in with Microsoft, the way the player expects: Sunrise asks for a
//! code, opens the browser, and waits. The code becomes a Microsoft token, that
//! token becomes an Xbox Live token, and that becomes the Minecraft session
//! Horizon's Launch API wants.
//!
//! Nobody has to set anything up first. The flow is the OAuth 2.0 device
//! authorization grant on Microsoft's consumer sign-in (`login.live.com`), with
//! Minecraft's own public title id as the application: `00000000402b5328` is the
//! id Minecraft registered for itself, and it is already allowed to reach Xbox
//! Live and Minecraft's services. So there is no app to create, no secret to
//! embed, no approval from Mojang to wait for, and no third-party launcher's id
//! borrowed. The token that comes back is scoped to Xbox Live only; turning it
//! into a Minecraft token is [`session_from_microsoft`]'s job.

use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::auth::{Checked, Session};
use crate::Result;

/// Minecraft's public title id on Microsoft's consumer sign-in service.
pub const CLIENT_ID: &str = "00000000402b5328";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const DEVICE_CODE: &str = "https://login.live.com/oauth20_connect.srf";
const TOKEN: &str = "https://login.live.com/oauth20_token.srf";
const XBL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";

/// What Microsoft shows the player: a short code and where to type it.
#[derive(Clone, Debug)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Microsoft's tokens from the device flow, before Xbox is involved.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// One poll of the token endpoint.
pub enum Poll {
    Pending,
    SlowDown,
    Ready(Tokens),
    Declined,
    Expired,
}

#[derive(Deserialize)]
struct DeviceAnswer {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("SunriseLauncher/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("could not start the HTTP client: {e}"))
}

fn api_error(body: &str) -> (Option<String>, Option<String>) {
    match serde_json::from_str::<Value>(body) {
        Ok(value) => (
            value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
            value
                .get("error_description")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        Err(_) => (None, None),
    }
}

fn describe_failure(status: u16, body: &str) -> String {
    let (error, description) = api_error(body);
    match error.as_deref() {
        Some("invalid_grant") | Some("invalid_client") | Some("unauthorized_client") => {
            "Microsoft's sign-in expired or was withdrawn; sign in again".to_string()
        }
        Some(other) => format!(
            "Microsoft refused the sign-in ({other}): {}",
            description
                .unwrap_or_default()
                .split(" Trace ID")
                .next()
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect::<String>()
        ),
        None => format!("Microsoft answered {status}"),
    }
}

fn parse_device(body: &str) -> Result<DeviceCode> {
    let answer: DeviceAnswer = serde_json::from_str(body).map_err(|e| {
        format!(
            "Microsoft's sign-in answer was not readable: {e} ({}): {body}",
            body.chars().take(120).collect::<String>()
        )
    })?;
    Ok(DeviceCode {
        device_code: answer.device_code,
        user_code: answer.user_code,
        verification_uri: answer.verification_uri,
        expires_in: answer.expires_in,
        interval: answer.interval.max(1),
    })
}

/// Ask Microsoft for a code to show the player. No account, no setup: this is
/// the first request of the flow and it works before anyone signs in.
pub fn begin() -> Result<DeviceCode> {
    let response = client()?
        .post(DEVICE_CODE)
        .form(&[
            ("client_id", CLIENT_ID),
            ("scope", SCOPE),
            ("response_type", "device_code"),
        ])
        .send()
        .map_err(|e| format!("could not reach Microsoft to start the sign-in: {e}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(describe_failure(status.as_u16(), &body));
    }
    parse_device(&body)
}

/// Poll the token endpoint once. The caller waits `interval` seconds between
/// calls; `slow_down` means it should wait longer.
pub fn poll(device_code: &str) -> Result<Poll> {
    let response = client()?
        .post(TOKEN)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
        ])
        .send()
        .map_err(|e| format!("could not reach Microsoft while waiting: {e}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();

    let answer: Option<TokenAnswer> = serde_json::from_str(&body).ok();
    if let Some(TokenAnswer {
        access_token: Some(token),
        refresh_token,
    }) = answer
    {
        return Ok(Poll::Ready(Tokens {
            access_token: token,
            refresh_token,
        }));
    }

    let (error, description) = api_error(&body);
    match error.as_deref() {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown),
        Some("authorization_declined") | Some("access_denied") => Ok(Poll::Declined),
        Some("expired_token") | Some("bad_verification_code") => Ok(Poll::Expired),
        Some(_) => Err(describe_failure(status.as_u16(), &body)),
        None => Err(format!(
            "Microsoft answered {status} without signing anyone in: {}",
            description.unwrap_or_default()
        )),
    }
}

/// Wait for the player to finish in the browser. `keep_going` is checked while
/// sleeping, so Cancel is answered within a quarter second.
pub fn wait(code: &DeviceCode, keep_going: impl Fn() -> bool) -> Result<Option<Tokens>> {
    let deadline = Instant::now() + Duration::from_secs(code.expires_in.max(60));
    let mut interval = code.interval.max(1);

    loop {
        if !keep_going() {
            return Ok(None);
        }
        let until = Instant::now() + Duration::from_secs(interval);
        while Instant::now() < until {
            if !keep_going() {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if Instant::now() >= deadline {
            return Err("the sign-in code expired; ask for a new one".to_string());
        }
        match poll(&code.device_code)? {
            Poll::Ready(tokens) => return Ok(Some(tokens)),
            Poll::Pending => {}
            Poll::SlowDown => interval += 5,
            Poll::Declined => return Err("the sign-in was declined in the browser".to_string()),
            Poll::Expired => return Err("the sign-in code expired; ask for a new one".to_string()),
        }
    }
}

/// A sentence for the XSTS numbers players actually hit.
fn xerr_hint(code: u64) -> String {
    match code {
        2148916233 => "this Microsoft account has no Xbox profile yet; make one at xbox.com, then sign in again".to_string(),
        2148916235 => "Xbox Live is not available in this account's country".to_string(),
        2148916236 | 2148916237 => "Xbox wants an adult to verify this account on xbox.com".to_string(),
        2148916238 => "this is a child account; an adult in its Microsoft family has to allow it to use apps like this".to_string(),
        other => format!("Xbox Live refused the sign-in (XErr {other})"),
    }
}

fn xui(value: &Value, key: &str) -> Option<String> {
    value
        .pointer(&format!("/DisplayClaims/xui/0/{key}"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The Microsoft token as an Xbox Live user token, plus the user hash the
/// Minecraft login needs.
fn xbox_live(msa_access_token: &str) -> Result<(String, String, Option<String>)> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={msa_access_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });
    let response = client()?
        .post(XBL)
        .json(&body)
        .send()
        .map_err(|e| format!("could not reach Xbox Live: {e}"))?;
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Xbox Live refused the Microsoft token ({status}); sign in again"
        ));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Xbox Live's answer was not readable: {e}"))?;
    let token = value
        .get("Token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Xbox Live's answer had no token".to_string())?
        .to_string();
    let uhs =
        xui(&value, "uhs").ok_or_else(|| "Xbox Live's answer had no user hash".to_string())?;
    Ok((token, uhs, xui(&value, "xid")))
}

/// The Xbox Live token as an XSTS token for Minecraft's relying party.
fn xsts(xbl_token: &str) -> Result<(String, String)> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });
    let response = client()?
        .post(XSTS)
        .json(&body)
        .send()
        .map_err(|e| format!("could not reach Xbox Live's token service: {e}"))?;
    let status = response.status();
    let text = response.text().unwrap_or_default();
    let value: Option<Value> = serde_json::from_str(&text).ok();

    if !status.is_success() {
        if let Some(code) = value
            .as_ref()
            .and_then(|v| v.get("XErr"))
            .and_then(Value::as_u64)
        {
            return Err(xerr_hint(code));
        }
        return Err(format!(
            "Xbox Live answered {status} without allowing Minecraft; try again in a minute"
        ));
    }

    let value = value.ok_or_else(|| "Xbox Live's answer was not readable".to_string())?;
    let token = value
        .get("Token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Xbox Live's answer had no token".to_string())?
        .to_string();
    let uhs =
        xui(&value, "uhs").ok_or_else(|| "Xbox Live's answer had no user hash".to_string())?;
    Ok((token, uhs))
}

/// The XSTS token as a Minecraft access token.
fn minecraft_token(uhs: &str, xsts_token: &str) -> Result<(String, String)> {
    let body = serde_json::json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") });
    let response = client()?
        .post(LOGIN)
        .json(&body)
        .send()
        .map_err(|e| format!("could not reach Minecraft's sign-in: {e}"))?;
    let status = response.status();
    let text = response.text().unwrap_or_default();

    if status.as_u16() == 403 && text.contains("app registration") {
        return Err(
            "Minecraft will not accept this sign-in right now. Meanwhile, import a session or play offline."
                .to_string(),
        );
    }
    if !status.is_success() {
        return Err(format!(
            "Minecraft refused the Xbox sign-in ({status}): {}",
            text.chars().take(160).collect::<String>()
        ));
    }

    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Minecraft's answer was not readable: {e}"))?;
    let token = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "Minecraft's answer had no access token".to_string())?
        .to_string();
    let name = value
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok((token, name))
}

/// The whole chain: Microsoft token in, a checked Minecraft session out.
///
/// `auth::check` runs at the end, so the session has a name, a uuid and a
/// confirmed ownership before Sunrise keeps it.
pub fn session_from_microsoft(
    msa_access_token: &str,
    refresh_token: Option<String>,
) -> Result<Checked> {
    let (xbl_token, _, xuid) = xbox_live(msa_access_token)?;
    let (xsts_token, uhs) = xsts(&xbl_token)?;
    let (mc_token, username) = minecraft_token(&uhs, &xsts_token)?;

    let draft = Session {
        name: username,
        access_token: mc_token,
        user_type: "msa".to_string(),
        xuid,
        issued_at: crate::now_secs(),
        refresh_token,
        ..Default::default()
    };
    crate::auth::check(&draft)
}

/// Renew a session from its refresh token: a new Microsoft token, then the same
/// chain again. Returns the session to keep, or a reason to sign in by hand.
pub fn refresh(session: &Session) -> Result<Checked> {
    let refresh_token = session
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            "this session was imported, so it cannot be renewed; sign in again".to_string()
        })?;

    let response = client()?
        .post(TOKEN)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .map_err(|e| format!("could not reach Microsoft to renew the sign-in: {e}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();

    let answer: TokenAnswer = serde_json::from_str(&body)
        .map_err(|e| format!("Microsoft's renewal answer was not readable: {e}"))?;
    let Some(access_token) = answer.access_token else {
        return Err(describe_failure(status.as_u16(), &body));
    };

    session_from_microsoft(
        &access_token,
        answer
            .refresh_token
            .or_else(|| Some(refresh_token.to_string())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsofts_code_answer_is_read() {
        // The shape login.live.com actually answers with, interval included.
        let body = r#"{"user_code":"NUJLB2VR","device_code":"abc","verification_uri":"https://www.microsoft.com/link","interval":5,"expires_in":900}"#;
        let code = parse_device(body).unwrap();
        assert_eq!(code.user_code, "NUJLB2VR");
        assert_eq!(code.verification_uri, "https://www.microsoft.com/link");
        assert_eq!(code.interval, 5);
        assert_eq!(code.expires_in, 900);
        assert!(parse_device(r#"{"error":"invalid_client"}"#).is_err());
    }

    #[test]
    fn xbox_refusals_read_like_sentences() {
        assert!(xerr_hint(2148916233).contains("no Xbox profile"));
        assert!(xerr_hint(2148916238).contains("child account"));
        assert!(xerr_hint(1234).contains("XErr 1234"));
    }

    #[test]
    fn a_refusal_says_to_sign_in_again() {
        let body = r#"{"error":"invalid_grant","error_description":"The refresh token has expired. Trace ID: 1"}"#;
        let message = describe_failure(400, body);
        assert!(message.contains("sign in again"));
        let other = describe_failure(400, r#"{"error":"slow_down"}"#);
        assert!(other.contains("slow_down"));
    }
}
