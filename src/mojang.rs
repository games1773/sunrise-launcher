//! The pinned Minecraft server jar, fetched once from Mojang.
//!
//! `horizon-prepare` needs the server jar for the first preparation and will not
//! download it itself. That is the one file Sunrise has to bring, so it brings it
//! from Mojang's public version manifest and checks it against the SHA-1 Horizon
//! pins in `research-lock.json`. The player never has to find or choose a jar.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::Result;

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone, Debug)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub message: String,
}

pub fn cache_dir(data: &Path) -> PathBuf {
    data.join("cache")
}

pub fn cache_path(data: &Path, version: &str) -> PathBuf {
    cache_dir(data).join(format!("{version}-server.jar"))
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("SunriseLauncher/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("could not start the HTTP client: {e}"))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// SHA-1 and size of a file, read in chunks so a 62 MB jar is not held in memory.
pub fn file_sha1(path: &Path) -> Result<(String, u64)> {
    let mut file =
        File::open(path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        size += read as u64;
    }
    Ok((hex(&hasher.finalize()), size))
}

/// The cached jar, but only when it still matches the pin.
pub fn cached(data: &Path, version: &str, pinned: &str) -> Option<PathBuf> {
    let path = cache_path(data, version);
    if !path.is_file() {
        return None;
    }
    match file_sha1(&path) {
        Ok((digest, _)) if !pinned.is_empty() && digest.eq_ignore_ascii_case(pinned) => Some(path),
        _ => None,
    }
}

/// What Mojang's version JSON says the server jar is.
pub fn server_download(version: &str) -> Result<(String, String, u64)> {
    let client = client()?;
    let manifest: Value = client
        .get(MANIFEST)
        .send()
        .map_err(|e| format!("could not reach Mojang's version list: {e}"))?
        .json()
        .map_err(|e| format!("Mojang's version list was not readable: {e}"))?;

    let url = manifest
        .get("versions")
        .and_then(Value::as_array)
        .and_then(|versions| {
            versions
                .iter()
                .find(|v| v.get("id").and_then(Value::as_str) == Some(version))
        })
        .and_then(|v| v.get("url"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Mojang's version list has no {version}"))?;

    let version_json: Value = client
        .get(url)
        .send()
        .map_err(|e| format!("could not reach Mojang's {version} version file: {e}"))?
        .json()
        .map_err(|e| format!("Mojang's {version} version file was not readable: {e}"))?;

    let server = version_json
        .get("downloads")
        .and_then(|d| d.get("server"))
        .ok_or_else(|| format!("Mojang's {version} version file lists no server jar"))?;
    let url = server
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "the server download has no url".to_string())?;
    let sha1 = server
        .get("sha1")
        .and_then(Value::as_str)
        .ok_or_else(|| "the server download has no sha1".to_string())?;
    let size = server.get("size").and_then(Value::as_u64).unwrap_or(0);
    Ok((url.to_string(), sha1.to_ascii_lowercase(), size))
}

/// The jar, downloaded and verified if the cache is empty or stale.
///
/// `pinned` is the SHA-1 from the build's `research-lock.json`; when Mojang
/// serves something else, Sunrise stops rather than handing the preparer a jar
/// Horizon did not pin.
pub fn ensure(
    data: &Path,
    version: &str,
    pinned: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf> {
    if let Some(path) = cached(data, version, pinned) {
        progress(Progress {
            done: 0,
            total: 0,
            message: format!("Using the {version} server jar already downloaded"),
        });
        return Ok(path);
    }

    progress(Progress {
        done: 0,
        total: 0,
        message: format!("Asking Mojang for the {version} server jar"),
    });
    let (url, sha1, size) = server_download(version)?;
    if !pinned.is_empty() && !sha1.eq_ignore_ascii_case(pinned) {
        return Err(format!(
            "Mojang now serves a different {version} server jar than Horizon pins\n  pinned  {pinned}\n  served  {sha1}\nSunrise will not pass it to the preparer."
        ));
    }

    crate::paths::ensure_dir(&cache_dir(data))?;
    let target = cache_path(data, version);
    let part = target.with_extension("jar.part");

    progress(Progress {
        done: 0,
        total: size,
        message: format!("Downloading the {version} server jar from Mojang"),
    });

    let mut response = client()?
        .get(&url)
        .send()
        .map_err(|e| format!("could not download the server jar: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Mojang answered {} for the server jar",
            response.status()
        ));
    }

    let total = response.content_length().unwrap_or(size);
    let mut file =
        File::create(&part).map_err(|e| format!("could not write {}: {e}", part.display()))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let read = response
            .read(&mut buf)
            .map_err(|e| format!("the server jar download stopped: {e}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buf[..read])
            .map_err(|e| format!("could not write the server jar: {e}"))?;
        hasher.update(&buf[..read]);
        done += read as u64;
        progress(Progress {
            done,
            total,
            message: format!("Downloading the {version} server jar from Mojang"),
        });
    }
    file.flush().ok();
    drop(file);

    let digest = hex(&hasher.finalize());
    if !digest.eq_ignore_ascii_case(&sha1) {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "the downloaded server jar does not match Mojang's own SHA-1\n  expected {sha1}\n  got      {digest}"
        ));
    }
    std::fs::rename(&part, &target).map_err(|e| format!("could not keep the server jar: {e}"))?;

    progress(Progress {
        done: total.max(done),
        total: total.max(done),
        message: format!("Kept the {version} server jar for the next preparation"),
    });
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_only_returns_a_jar_that_matches_the_pin() {
        let dir = std::env::temp_dir().join(format!("sunrise-jar-{}", crate::now_secs()));
        std::fs::create_dir_all(cache_dir(&dir)).unwrap();
        let path = cache_path(&dir, "26.3");
        std::fs::write(&path, b"not a real jar").unwrap();
        let (digest, size) = file_sha1(&path).unwrap();
        assert_eq!(size, "not a real jar".len() as u64);
        assert!(cached(&dir, "26.3", &digest).is_some());
        assert!(cached(&dir, "26.3", "0000000000000000000000000000000000000000").is_none());
        assert!(cached(&dir, "26.4", &digest).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha1_matches_the_known_vector() {
        let dir = std::env::temp_dir().join(format!("sunrise-sha1-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("abc");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            file_sha1(&path).unwrap().0,
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
