use anyhow::{Context, Result, bail};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

const DX_VERSION: &str = "0.7.10";
const USAGE: &str = "Usage: cargo xtask dev";

fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [command] if command == "dev" => dev(),
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

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let status = Command::new(dx)
        .current_dir(root)
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
