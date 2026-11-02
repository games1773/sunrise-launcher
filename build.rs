//! Draws the Sunrise mark into a Windows icon and links it into the binaries,
//! so the .exe carries the same logo the window shows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

include!("src/bin/sunrise/logo_grid.rs");

const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bin/sunrise/logo_grid.rs");

    let ico = build_ico();
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let copy = manifest_dir.join("assets").join("sunrise.ico");
    if fs::read(&copy).ok().as_deref() != Some(ico.as_slice()) {
        let _ = fs::create_dir_all(copy.parent().unwrap());
        let _ = fs::write(&copy, &ico);
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out_dir.join("sunrise.ico");
    fs::write(&ico_path, &ico).expect("write the icon");
    link_into_exes(&out_dir, &ico_path);
}

fn color(index: usize) -> [u8; 4] {
    let (r, g, b) = LOGO_PALETTE[index];
    if index == 0 {
        [0, 0, 0, 0]
    } else {
        [r, g, b, 255]
    }
}

/// One pixel of the mark, in the icon's own resolution.
fn sample(x: u32, y: u32, size: u32) -> [u8; 4] {
    let gx = (x * LOGO_W as u32 / size) as usize;
    let gy = (y * LOGO_H as u32 / size) as usize;
    let index = (LOGO_ART[gy].as_bytes()[gx] as u8).wrapping_sub(b'0') as usize;
    color(index.min(LOGO_PALETTE.len() - 1))
}

/// A 32-bit bitmap entry: header, bottom-up BGRA pixels, then an empty mask.
fn bitmap(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4 + size * 8) as usize);
    let put = |out: &mut Vec<u8>, value: u32| out.extend_from_slice(&value.to_le_bytes());

    put(&mut out, 40); // biSize
    put(&mut out, size); // biWidth
    put(&mut out, size * 2); // biHeight, doubled for the mask
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    put(&mut out, 0); // BI_RGB
    put(&mut out, size * size * 4); // biSizeImage
    for _ in 0..4 {
        put(&mut out, 0); // pixels per metre, palette counts
    }

    for y in (0..size).rev() {
        for x in 0..size {
            let [r, g, b, a] = sample(x, y, size);
            out.extend_from_slice(&[b, g, r, a]);
        }
    }

    // The AND mask is unused at 32 bits, but Windows expects it to be there.
    let mask_row = size.div_ceil(32) * 4;
    out.resize(out.len() + (mask_row * size) as usize, 0);
    out
}

fn build_ico() -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|&s| (s, bitmap(s))).collect();

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in &images {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        out.push(dimension);
        out.push(dimension);
        out.push(0); // palette entries
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in &images {
        out.extend_from_slice(data);
    }
    out
}

/// Compile a one-line resource script with windres and hand the object to the
/// linker. Without a resource compiler the build still succeeds, icon-less.
fn link_into_exes(out_dir: &Path, ico: &Path) {
    let script = out_dir.join("sunrise.rc");
    let text = format!(
        "1 ICON \"{}\"\n",
        ico.display().to_string().replace('\\', "\\\\")
    );
    if fs::write(&script, text).is_err() {
        println!("cargo:warning=could not write the icon resource script");
        return;
    }
    let object = out_dir.join("sunrise_res.o");
    let built = Command::new("windres")
        .arg("-O")
        .arg("coff")
        .arg("-i")
        .arg(&script)
        .arg("-o")
        .arg(&object)
        .output();
    match built {
        Ok(output) if output.status.success() => {
            println!("cargo:rustc-link-arg-bins={}", object.display());
        }
        Ok(output) => println!(
            "cargo:warning=windres failed, building without an exe icon: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(err) => {
            println!("cargo:warning=windres not found, building without an exe icon: {err}")
        }
    }
}
