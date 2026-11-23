use std::{env, fs, io::Write, path::Path};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign::{KeyPair, PublicKeyBox, SecretKeyBox};
use serde_json::{Map, json};

const ASSETS: [(&str, &str, &str); 3] = [
    (
        "linux-x86_64",
        "corner-table-linux-x86_64.AppImage",
        "appimage",
    ),
    (
        "windows-x86_64",
        "corner-table-windows-x86_64-setup.exe",
        "nsis",
    ),
    (
        "macos-aarch64",
        "corner-table-macos-arm64.app.tar.gz",
        "app",
    ),
];

pub fn key_pair(directory: &Path) -> Result<()> {
    let password =
        env::var("CORNER_TABLE_UPDATE_PASSWORD").context("set CORNER_TABLE_UPDATE_PASSWORD")?;
    ensure!(!password.is_empty(), "use a password for the release key");
    let keys = KeyPair::generate_encrypted_keypair(Some(password))?;
    let mut private = fs::OpenOptions::new();
    private.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        private.mode(0o600);
    }
    private.open(directory.join("updater.key"))?.write_all(
        STANDARD
            .encode(keys.sk.to_box(None)?.to_string())
            .as_bytes(),
    )?;
    fs::write(
        directory.join("updater.pub"),
        STANDARD.encode(keys.pk.to_box()?.to_string()),
    )?;
    Ok(())
}

pub fn sign(root: &Path, dist: &Path, version: &str, repository: &str) -> Result<()> {
    let version = semver::Version::parse(version)?;
    ensure!(
        version.to_string() == env!("CARGO_PKG_VERSION"),
        "release version must match Cargo.toml"
    );
    let secret = env::var("CORNER_TABLE_UPDATE_KEY").context("read CORNER_TABLE_UPDATE_KEY")?;
    let password =
        env::var("CORNER_TABLE_UPDATE_PASSWORD").context("read CORNER_TABLE_UPDATE_PASSWORD")?;
    let secret = SecretKeyBox::from_string(&String::from_utf8(STANDARD.decode(secret.trim())?)?)?
        .into_secret_key(Some(password))?;
    let public = fs::read_to_string(root.join("packaging/updater.pub"))?;
    let public = PublicKeyBox::from_string(&String::from_utf8(STANDARD.decode(public.trim())?)?)?
        .into_public_key()?;
    let base = format!("https://github.com/{repository}/releases/download/v{version}");
    let mut platforms = Map::new();
    for (platform, filename, format) in ASSETS {
        let package = dist.join(filename);
        let signed = minisign::sign(
            Some(&public),
            &secret,
            fs::File::open(&package).with_context(|| format!("open {}", package.display()))?,
            Some(&format!("version:{version}\tfile:{filename}")),
            None,
        )?;
        minisign::verify(
            &public,
            &signed,
            fs::File::open(&package)?,
            true,
            false,
            false,
        )?;
        let signature = STANDARD.encode(signed.to_string());
        fs::write(dist.join(format!("{filename}.sig")), &signature)?;
        platforms.insert(
            platform.into(),
            json!({
                "url": format!("{base}/{filename}"),
                "signature": signature,
                "format": format,
            }),
        );
    }
    let manifest = json!({"version": version.to_string(), "platforms": platforms});
    fs::write(
        dist.join("latest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    Ok(())
}
