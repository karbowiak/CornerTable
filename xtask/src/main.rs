#[cfg(target_os = "macos")]
mod macos;
mod release;

use anyhow::{Context, Result, bail};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

const DX_VERSION: &str = "0.7.10";
const USAGE: &str = "Usage: cargo xtask <dev [--restart] | bundle | update-key DIR | sign-updates DIST VERSION REPO>\n\
    \n\
    dev             Start the Dioxus hot reload server\n\
    dev --restart   Rebuild and relaunch on source or asset changes (Python 3)\n\
    bundle          Build target/Corner Table.app (macOS)\n\
    update-key DIR  Generate a Corner Table signing key pair in a private directory\n\
    sign-updates    Sign updater packages and write latest.json";

fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [command] if command == "dev" => dev(),
        [command, flag] if command == "dev" && flag == "--restart" => dev_restart(),
        [command] if command == "bundle" => bundle(),
        [command, dir] if command == "update-key" => release::key_pair(Path::new(dir)),
        [command, dir, version, repo] if command == "sign-updates" => release::sign(
            workspace_root(),
            Path::new(dir),
            version.to_str().context("release version must be UTF-8")?,
            repo.to_str().context("repository name must be UTF-8")?,
        ),
        [] => {
            println!("{USAGE}");
            Ok(())
        }
        [flag] if flag == "-h" || flag == "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
}

fn bundle() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::bundle(workspace_root());
    #[cfg(not(target_os = "macos"))]
    bail!("cargo xtask bundle requires macOS and Apple's Command Line Tools");
}

fn dev_restart() -> Result<()> {
    let status = Command::new("python3")
        .current_dir(workspace_root())
        .arg("scripts/dev.py")
        .status()
        .context("start the rebuild watcher with python3; install Python 3 if it is missing")?;
    if !status.success() {
        bail!("rebuild watcher exited with {status}");
    }
    Ok(())
}

fn dev() -> Result<()> {
    let dx = dx_path();
    let version = Command::new(&dx)
        .arg("--version")
        .output()
        .with_context(|| {
            format!(
                "run {}; install Dioxus CLI {DX_VERSION} or set CORNER_TABLE_DX",
                dx.display()
            )
        })?;
    if !version.status.success()
        || String::from_utf8_lossy(&version.stdout)
            .split_whitespace()
            .take(2)
            .ne(["dioxus", DX_VERSION])
    {
        bail!(
            "{} must be Dioxus CLI {DX_VERSION}; set CORNER_TABLE_DX to its executable",
            dx.display()
        );
    }

    let status = Command::new(dx)
        .current_dir(workspace_root())
        // Dioxus 0.7.10 invalidates the legacy Cargo fingerprint before each full relink.
        // Keep that layout for its builds until Dioxus supports Cargo 1.100's new cache.
        .env("__CARGO_TEMPORARY_BUILD_DIR_NEW_LAYOUT_OPT_OUT", "1")
        .args([
            "serve",
            "--platform",
            env::consts::OS,
            "--package",
            "corner-table-dev",
            "--bin",
            "corner-table-dev",
            "--features",
            "hot-reload",
            "--hot-patch",
            "--addr",
            "127.0.0.1",
            "--open",
            "false",
            "--locked",
        ])
        .status()
        .context("run the Dioxus development server")?;
    if !status.success() {
        bail!("Dioxus development server exited with {status}");
    }
    Ok(())
}

fn dx_path() -> PathBuf {
    if let Some(path) = env::var_os("CORNER_TABLE_DX") {
        return path.into();
    }
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".cargo")));
    if let Some(path) = cargo_home.map(|home| {
        home.join("bin")
            .join(format!("dx{}", env::consts::EXE_SUFFIX))
    }) && path.is_file()
    {
        return path;
    }
    "dx".into()
}
