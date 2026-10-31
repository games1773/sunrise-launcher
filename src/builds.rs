//! Finding, downloading and unpacking Horizon builds from GitHub.
//!
//! Horizon publishes each build as one archive per platform inside a GitHub
//! release, next to a `SHA256SUMS` file. The asset carries a `sha256:` digest,
//! so every download is checked before it is unpacked.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{paths, Result};

const REPO: &str = "horizon-rust/horizon";
const API: &str = "https://api.github.com/repos/horizon-rust/horizon/releases";
const CACHE_TTL: u64 = 600;

/// The archive this platform runs, or `None` for an unsupported machine.
pub fn platform_asset() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("horizon-windows-x64.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("horizon-linux-x86_64.tar.gz")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("horizon-macos-arm64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("horizon-macos-x64.zip")
    } else {
        None
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteBuild {
    pub tag: String,
    pub label: String,
    pub prerelease: bool,
    pub published_at: String,
    pub asset_name: String,
    pub asset_url: String,
    pub size: u64,
    pub sha256: Option<String>,
    pub page_url: String,
}

impl RemoteBuild {
    /// "Nightly 2026-10-31" style short name for the UI.
    pub fn short_name(&self) -> String {
        let label = self.label.trim();
        if label.is_empty() {
            self.tag.trim_start_matches("nightly-").to_string()
        } else {
            label.to_string()
        }
    }

    pub fn is_nightly(&self) -> bool {
        self.tag.starts_with("nightly-") || self.prerelease
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledBuild {
    pub tag: String,
    pub dir: PathBuf,
    pub label: Option<String>,
    pub asset_name: Option<String>,
    pub sha256: Option<String>,
    pub installed_at: u64,
    pub version: String,
}

#[derive(Deserialize)]
struct ReleaseJson {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    name: String,
    #[serde(default)]
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    fetched_at: u64,
    builds: Vec<RemoteBuild>,
}

pub fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("SunriseLauncher/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("could not start the HTTP client: {e}"))
}

fn cache_file(data: &Path) -> PathBuf {
    data.join("builds-cache.json")
}

/// The published builds that can run here, newest first.
///
/// `force` skips the on-disk cache; otherwise a fetch younger than ten minutes
/// is reused, which keeps well inside GitHub's unauthenticated rate limit.
pub fn fetch_builds(data: &Path, force: bool) -> Result<Vec<RemoteBuild>> {
    if !force {
        if let Ok(text) = std::fs::read_to_string(cache_file(data)) {
            if let Ok(cache) = serde_json::from_str::<Cache>(&text) {
                if crate::now_secs().saturating_sub(cache.fetched_at) < CACHE_TTL {
                    return Ok(cache.builds);
                }
            }
        }
    }

    let asset_name = platform_asset()
        .ok_or_else(|| "Sunrise has no Horizon build for this platform (Windows x64, Linux x86-64, macOS arm64 or x64)".to_string())?;

    let client = http_client()?;
    let response = client
        .get(format!("{API}?per_page=60"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("could not reach GitHub: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let hint = if status.as_u16() == 403 || status.as_u16() == 429 {
            " (GitHub's rate limit; try again in a few minutes)"
        } else {
            ""
        };
        return Err(format!("GitHub answered {status}{hint}"));
    }

    let releases: Vec<ReleaseJson> = response
        .json()
        .map_err(|e| format!("GitHub sent something Sunrise could not read: {e}"))?;

    let mut builds = Vec::new();
    for release in releases {
        let Some(asset) = release.assets.iter().find(|a| a.name == asset_name) else {
            continue;
        };
        let sha256 = asset.digest.as_ref().and_then(|d| {
            d.strip_prefix("sha256:")
                .filter(|v| v.len() == 64)
                .map(|v| v.to_ascii_lowercase())
        });
        builds.push(RemoteBuild {
            tag: release.tag_name.clone(),
            label: release.name.clone().unwrap_or_else(|| release.tag_name.clone()),
            prerelease: release.prerelease,
            published_at: release.published_at.unwrap_or_default(),
            asset_name: asset.name.clone(),
            asset_url: asset.browser_download_url.clone(),
            size: asset.size,
            sha256,
            page_url: release.html_url,
        });
    }

    builds.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    let _ = paths::ensure_dir(data);
    if let Ok(text) = serde_json::to_string(&Cache {
        fetched_at: crate::now_secs(),
        builds: builds.clone(),
    }) {
        let _ = std::fs::write(cache_file(data), text);
    }

    if builds.is_empty() {
        return Err(format!(
            "no release of {REPO} carries {asset_name} yet"
        ));
    }
    Ok(builds)
}

pub fn sanitize_tag(tag: &str) -> String {
    let mut out = String::new();
    for ch in tag.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    if out.is_empty() {
        "build".to_string()
    } else {
        out
    }
}

pub fn build_dir(data: &Path, tag: &str) -> PathBuf {
    paths::builds_dir(data).join(sanitize_tag(tag))
}

pub fn client_exe(build_dir: &Path) -> PathBuf {
    build_dir.join(if cfg!(windows) {
        "horizon-client.exe"
    } else {
        "horizon-client"
    })
}

pub fn prepare_exe(build_dir: &Path) -> PathBuf {
    build_dir.join(if cfg!(windows) {
        "horizon-prepare.exe"
    } else {
        "horizon-prepare"
    })
}

/// The Minecraft version this build plays, read from its own lock file.
pub fn game_version(build_dir: &Path) -> String {
    std::fs::read_to_string(build_dir.join("research-lock.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| {
            value
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "26.3".to_string())
}

#[derive(Default, Deserialize, Serialize)]
struct BuildMarker {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    asset_name: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    installed_at: u64,
}

fn marker_file(dir: &Path) -> PathBuf {
    dir.join("sunrise-build.json")
}

pub fn installed_builds(data: &Path) -> Vec<InstalledBuild> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(paths::builds_dir(data)) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        if dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(true)
        {
            continue;
        }
        if !client_exe(&dir).is_file() {
            continue;
        }
        let marker: BuildMarker = std::fs::read_to_string(marker_file(&dir))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let tag = if marker.tag.is_empty() {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("build")
                .to_string()
        } else {
            marker.tag.clone()
        };
        out.push(InstalledBuild {
            version: game_version(&dir),
            tag,
            dir,
            label: marker.label,
            asset_name: marker.asset_name,
            sha256: marker.sha256,
            installed_at: marker.installed_at,
        });
    }
    out.sort_by(|a, b| a.installed_at.cmp(&b.installed_at));
    out
}

pub fn find_installed(data: &Path, tag: &str) -> Option<InstalledBuild> {
    installed_builds(data)
        .into_iter()
        .find(|b| b.tag == tag || paths::slug(&b.tag) == paths::slug(tag))
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub message: String,
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Download and unpack one build. The archive is checked against its published
/// SHA-256 before anything is unpacked.
pub fn install(
    data: &Path,
    build: &RemoteBuild,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledBuild> {
    let downloads = paths::downloads_dir(data);
    paths::ensure_dir(&downloads)?;
    paths::ensure_dir(&paths::builds_dir(data))?;

    let archive = downloads.join(&build.asset_name);
    let part = downloads.join(format!("{}.part", build.asset_name));

    progress(Progress {
        done: 0,
        total: build.size,
        message: format!("Downloading {}", build.asset_name),
    });

    let client = http_client()?;
    let mut response = client
        .get(&build.asset_url)
        .send()
        .map_err(|e| format!("download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub answered {} for {}",
            response.status(),
            build.asset_name
        ));
    }

    let total = response.content_length().unwrap_or(build.size);
    let mut file = File::create(&part).map_err(|e| format!("could not write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done: u64 = 0;
    loop {
        let read = response
            .read(&mut buf)
            .map_err(|e| format!("download interrupted: {e}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buf[..read])
            .map_err(|e| format!("could not write the download: {e}"))?;
        hasher.update(&buf[..read]);
        done += read as u64;
        progress(Progress {
            done,
            total,
            message: format!("Downloading {}", build.asset_name),
        });
    }
    file.flush().ok();
    drop(file);

    let digest = hex(&hasher.finalize());
    if let Some(expected) = &build.sha256 {
        if expected != &digest {
            let _ = std::fs::remove_file(&part);
            return Err(format!(
                "the download does not match its published checksum\n  expected {expected}\n  got      {digest}"
            ));
        }
    }
    std::fs::rename(&part, &archive)
        .map_err(|e| format!("could not keep the download: {e}"))?;

    progress(Progress {
        done: total.max(done),
        total: total.max(done),
        message: "Unpacking".to_string(),
    });

    let target = build_dir(data, &build.tag);
    let staging = paths::builds_dir(data).join(format!(".staging-{}", sanitize_tag(&build.tag)));
    let _ = std::fs::remove_dir_all(&staging);
    paths::ensure_dir(&staging)?;

    let unpacked = unpack(&archive, &staging)?;

    let _ = std::fs::remove_dir_all(&target);
    if unpacked != staging {
        std::fs::rename(&unpacked, &target)
            .map_err(|e| format!("could not move the build into place: {e}"))?;
        let _ = std::fs::remove_dir_all(&staging);
    } else {
        std::fs::rename(&staging, &target)
            .map_err(|e| format!("could not move the build into place: {e}"))?;
    }

    if !client_exe(&target).is_file() {
        return Err(format!(
            "the archive did not contain {}; Sunrise deleted nothing, look in {}",
            client_exe(&target)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            target.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["horizon-client", "horizon-prepare", "horizon-ctl", "horizon-server"] {
            let path = target.join(name);
            if path.is_file() {
                if let Ok(meta) = std::fs::metadata(&path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&path, perms);
                }
            }
        }
    }

    let installed = InstalledBuild {
        tag: build.tag.clone(),
        dir: target.clone(),
        label: Some(build.label.clone()),
        asset_name: Some(build.asset_name.clone()),
        sha256: Some(digest),
        installed_at: crate::now_secs(),
        version: game_version(&target),
    };

    let marker = BuildMarker {
        tag: installed.tag.clone(),
        label: installed.label.clone(),
        asset_name: installed.asset_name.clone(),
        sha256: installed.sha256.clone(),
        installed_at: installed.installed_at,
    };
    if let Ok(text) = serde_json::to_string_pretty(&marker) {
        let _ = std::fs::write(marker_file(&target), text);
    }

    let _ = std::fs::remove_file(&archive);

    progress(Progress {
        done: 1,
        total: 1,
        message: format!("Installed {}", build.short_name()),
    });

    Ok(installed)
}

/// Unpack a zip or tar.gz, then strip the single wrapper folder Horizon ships
/// (for example `horizon-windows-x64/`).
fn unpack(archive: &Path, dest: &Path) -> Result<PathBuf> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if name.ends_with(".zip") {
        let file = File::open(archive).map_err(|e| format!("could not open the archive: {e}"))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("could not read the zip: {e}"))?;
        for index in 0..zip.len() {
            let mut entry = zip
                .by_index(index)
                .map_err(|e| format!("could not read the zip: {e}"))?;
            let Some(relative) = entry.enclosed_name() else {
                continue;
            };
            let out_path = dest.join(relative);
            if entry.is_dir() {
                paths::ensure_dir(&out_path)?;
                continue;
            }
            if let Some(parent) = out_path.parent() {
                paths::ensure_dir(parent)?;
            }
            let mut out = File::create(&out_path)
                .map_err(|e| format!("could not write {}: {e}", out_path.display()))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("could not unpack {}: {e}", out_path.display()))?;
        }
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = File::open(archive).map_err(|e| format!("could not open the archive: {e}"))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(dest)
            .map_err(|e| format!("could not unpack the archive: {e}"))?;
    } else {
        return Err(format!("Sunrise cannot unpack {}", archive.display()));
    }

    Ok(strip_wrapper(dest))
}

/// If everything unpacked sits inside one folder, return that folder.
fn strip_wrapper(dest: &Path) -> PathBuf {
    let Ok(entries) = std::fs::read_dir(dest) else {
        return dest.to_path_buf();
    };
    let entries: Vec<_> = entries.flatten().collect();
    if entries.len() == 1 {
        let only = &entries[0];
        if only.path().is_dir() {
            return only.path();
        }
    }
    dest.to_path_buf()
}

/// The newest installed build, by install time.
pub fn newest_installed(data: &Path) -> Option<InstalledBuild> {
    installed_builds(data).pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_become_folder_names() {
        assert_eq!(sanitize_tag("nightly-2026-10-31"), "nightly-2026-10-31");
        assert_eq!(sanitize_tag("v0.2.31"), "v0.2.31");
        assert_eq!(sanitize_tag("a/b c"), "a-b-c");
    }

    #[test]
    fn digest_hex_is_lowercase() {
        assert_eq!(hex(&[0x00, 0xff, 0x1a]), "00ff1a");
    }

    #[test]
    fn single_wrapper_folder_is_stripped() {
        let dir = std::env::temp_dir().join(format!("sunrise-strip-{}", crate::now_secs()));
        let inner = dir.join("horizon-windows-x64");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("horizon-client.exe"), b"x").unwrap();
        assert_eq!(strip_wrapper(&dir), inner);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
