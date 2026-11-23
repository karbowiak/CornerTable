use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const APPLICATION: &str = "Corner Table.app";
const EXECUTABLE: &str = "corner-table";
const ICON: &str = "AppIcon.icns";
const PLIST: &str = include_str!("../../packaging/macos/Info.plist");

pub fn bundle(root: &Path) -> Result<()> {
    let (executable, version) = build(root)?;
    let target = root.join("target");
    fs::create_dir_all(&target).context("create the workspace target directory")?;
    let staging = tempfile::Builder::new()
        .prefix(".corner-table-")
        .tempdir_in(&target)
        .context("stage the macOS bundle")?;
    let staged_app = staging.path().join(APPLICATION);
    let contents = staged_app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos).context("create the bundle executable directory")?;
    fs::create_dir_all(&resources).context("create the bundle resources directory")?;
    let binary = macos.join(EXECUTABLE);
    fs::copy(&executable, &binary)
        .with_context(|| format!("copy the executable from {}", executable.display()))?;

    let minimum = minimum_system_version(&binary)?;
    let plist = contents.join("Info.plist");
    fs::write(
        &plist,
        PLIST
            .replace("@VERSION@", &version)
            .replace("@MINIMUM_SYSTEM_VERSION@", &minimum),
    )
    .context("write the bundle Info.plist")?;

    let source_icon = root.join("packaging/macos/icon.png");
    match source_icon.symlink_metadata() {
        Ok(_) => {
            build_icon(&source_icon, staging.path(), &resources.join(ICON))?;
            run(Command::new("plutil")
                .args(["-insert", "CFBundleIconFile", "-string", ICON])
                .arg(&plist))?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            eprintln!(
                "No packaging/macos/icon.png found. Using the default macOS app icon. \
                 Add a 1024×1024 PNG there and rerun `cargo xtask bundle` to build the custom icon."
            );
        }
        Err(error) => return Err(error).context("read packaging/macos/icon.png"),
    }
    run(Command::new("plutil").arg("-lint").arg(&plist))?;

    let destination = target.join(APPLICATION);
    let lock = fs::File::create(target.join(".corner-table-bundle.lock"))?;
    lock.lock().context("lock the bundle destination")?;
    // Keep the previous bundle until the new executable, icon, and metadata are ready.
    let previous = staging.path().join("previous.app");
    let had_previous = match fs::rename(&destination, &previous) {
        Ok(()) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => return Err(error).context("move the previous macOS bundle"),
    };
    if let Err(error) = fs::rename(&staged_app, &destination) {
        if had_previous && let Err(restore_error) = fs::rename(&previous, &destination) {
            let saved = staging.keep().join("previous.app");
            bail!(
                "install the macOS bundle: {error}; restore failed: {restore_error}; \
                 the previous bundle is at {}",
                saved.display()
            );
        }
        return Err(error).context("install the macOS bundle");
    }

    println!("Built {}", destination.display());
    Ok(())
}

fn build(root: &Path) -> Result<(PathBuf, String)> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let metadata = run(Command::new(&cargo).current_dir(root).args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--locked",
    ]))?;
    let metadata: Value = serde_json::from_str(&metadata).context("read Cargo metadata")?;
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["name"] == EXECUTABLE)
        })
        .context("find Corner Table in Cargo metadata")?;
    let version = package["version"]
        .as_str()
        .context("read the app version")?;
    // Apple's version fields take numeric components, so keep the SemVer core.
    let version = version.split(['-', '+']).next().unwrap_or(version);

    let build = Command::new(cargo)
        .current_dir(root)
        .stderr(Stdio::inherit())
        .args([
            "build",
            "--locked",
            "--release",
            "--package",
            EXECUTABLE,
            "--bin",
            EXECUTABLE,
            "--message-format=json-render-diagnostics",
        ])
        .output()
        .context("run Cargo to build Corner Table")?;
    ensure!(
        build.status.success(),
        "Cargo build exited with {}",
        build.status
    );
    let artifacts = String::from_utf8_lossy(&build.stdout);
    let executable = artifacts
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|message| {
            if message["reason"] == "compiler-artifact"
                && message["package_id"] == package["id"]
                && message["target"]["name"] == EXECUTABLE
            {
                return message["executable"].as_str().map(PathBuf::from);
            }
            None
        })
        .context("find the built Corner Table executable in Cargo's output")?;
    Ok((executable, version.into()))
}

fn minimum_system_version(binary: &Path) -> Result<String> {
    let load_commands = run(Command::new("xcrun")
        .args(["--sdk", "macosx", "vtool", "-show-build"])
        .arg(binary))?;
    let key = if load_commands.contains("LC_VERSION_MIN_MACOSX") {
        "version"
    } else {
        "minos"
    };
    load_commands
        .lines()
        .filter_map(|line| line.trim().split_once(char::is_whitespace))
        .find_map(|(field, value)| (field == key).then_some(value.trim()))
        .map(String::from)
        .context("read the linked minimum macOS version from the executable")
}

fn build_icon(source: &Path, staging: &Path, output: &Path) -> Result<()> {
    let info = run(Command::new("sips")
        .args(["-g", "format", "-g", "pixelWidth", "-g", "pixelHeight"])
        .arg(source))?;
    let has_property = |name, value| {
        info.lines()
            .any(|line| line.trim().split_once(": ") == Some((name, value)))
    };
    ensure!(
        has_property("format", "png")
            && has_property("pixelWidth", "1024")
            && has_property("pixelHeight", "1024"),
        "packaging/macos/icon.png must be a 1024×1024 PNG"
    );

    let iconset = staging.join("AppIcon.iconset");
    fs::create_dir(&iconset).context("stage the macOS iconset")?;
    for (pixels, filenames) in [
        ("16", &["icon_16x16.png"][..]),
        ("32", &["icon_16x16@2x.png", "icon_32x32.png"]),
        ("64", &["icon_32x32@2x.png"]),
        ("128", &["icon_128x128.png"]),
        ("256", &["icon_128x128@2x.png", "icon_256x256.png"]),
        ("512", &["icon_256x256@2x.png", "icon_512x512.png"]),
    ] {
        let resized = iconset.join(filenames[0]);
        run(Command::new("sips")
            .args(["-z", pixels, pixels])
            .arg(source)
            .arg("--out")
            .arg(&resized))?;
        if let Some(second) = filenames.get(1) {
            fs::copy(resized, iconset.join(second)).context("copy the doubled icon size")?;
        }
    }
    fs::copy(source, iconset.join("icon_512x512@2x.png")).context("copy the full-size icon")?;
    run(Command::new("iconutil")
        .args(["--convert", "icns"])
        .arg(&iconset)
        .arg("--output")
        .arg(output))?;
    Ok(())
}

fn run(command: &mut Command) -> Result<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().with_context(|| format!("run {program}"))?;
    ensure!(
        output.status.success(),
        "{program} exited with {}\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
