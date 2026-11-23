use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cargo_packager_updater::{
    Config, Update, UpdaterBuilder, WindowsConfig, WindowsUpdateInstallMode,
};
use gpui::{App, AsyncWindowContext, PromptLevel, Window};
use minisign_verify::Signature;

use crate::app::CheckForUpdates;

const RELEASES: &str = "https://github.com/karbowiak/CornerTable/releases";
const MANIFEST: &str =
    "https://github.com/karbowiak/CornerTable/releases/latest/download/latest.json";
const PUBLIC_KEY: &str = include_str!("../../../packaging/updater.pub");
static CHECKING: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);

struct Check;

impl Drop for Check {
    fn drop(&mut self) {
        CHECKING.store(false, Ordering::Release);
    }
}

pub fn check_at_startup(window: &mut Window, cx: &mut App) {
    if !STARTED.swap(true, Ordering::Relaxed) {
        check(window, cx, false);
    }
}

pub fn check_for_updates(_: &CheckForUpdates, window: &mut Window, cx: &mut App) {
    check(window, cx, true);
}

fn check(window: &mut Window, cx: &mut App, manual: bool) {
    if option_env!("CORNER_TABLE_RELEASE_BUILD") != Some("1") {
        if manual {
            cx.open_url(RELEASES);
        }
        return;
    }
    if CHECKING.swap(true, Ordering::AcqRel) {
        return;
    }
    let check = Check;
    window
        .spawn(cx, async move |cx| {
            let _check = check;
            if !manual {
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
            match cx
                .background_executor()
                .spawn(async { find_update() })
                .await
            {
                Ok(Some(update)) => offer(update, cx).await,
                Ok(None) if manual => {
                    let _ = cx
                        .prompt(
                            PromptLevel::Info,
                            "Corner Table is up to date",
                            None,
                            &["OK"],
                        )
                        .await;
                }
                Err(error) if manual => {
                    offer_download("Couldn’t check for updates", &error.to_string(), cx).await
                }
                _ => {}
            }
        })
        .detach();
}

fn find_update() -> Result<Option<Update>> {
    let Some(asset) = package_name() else {
        return Ok(None);
    };
    let config = Config {
        endpoints: vec![MANIFEST.parse()?],
        pubkey: PUBLIC_KEY.trim().to_owned(),
        windows: Some(WindowsConfig {
            install_mode: Some(WindowsUpdateInstallMode::BasicUi),
            ..WindowsConfig::default()
        }),
    };
    let updater = UpdaterBuilder::new(semver::Version::parse(env!("CARGO_PKG_VERSION"))?, config)
        .timeout(Duration::from_secs(20))
        .build()?;
    let Some(mut update) = updater.check()? else {
        return Ok(None);
    };
    let version = semver::Version::parse(&update.version)?;
    if !version.pre.is_empty() {
        return Ok(None);
    }
    ensure!(
        update.download_url.as_str() == format!("{RELEASES}/download/v{version}/{asset}"),
        "Unexpected update URL"
    );
    ensure!(
        update.format.to_string() == package_format(),
        "Unexpected update format"
    );
    let signature = Signature::decode(&String::from_utf8(STANDARD.decode(&update.signature)?)?)?;
    ensure!(
        signature.trusted_comment() == format!("version:{version}\tfile:{asset}"),
        "Update signature names a different version or package"
    );
    update.timeout = Some(Duration::from_secs(900));
    Ok(Some(update))
}

fn package_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("corner-table-macos-arm64.app.tar.gz"),
        ("windows", "x86_64") => Some("corner-table-windows-x86_64-setup.exe"),
        ("linux", "x86_64") => Some("corner-table-linux-x86_64.AppImage"),
        _ => None,
    }
}

fn package_format() -> &'static str {
    match std::env::consts::OS {
        "macos" => "app",
        "windows" => "nsis",
        _ => "appimage",
    }
}

fn installed_path() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
        return (path.is_absolute() && path.symlink_metadata().ok()?.is_file()).then_some(path);
    }
    let executable = std::env::current_exe().ok()?;
    if cfg!(windows) {
        return executable
            .parent()?
            .join("Uninstall.exe")
            .is_file()
            .then_some(executable);
    }
    let bundle = executable.ancestors().nth(3)?;
    (bundle.extension()?.eq_ignore_ascii_case("app") && !bundle.starts_with("/Volumes"))
        .then(|| bundle.to_owned())
}

async fn offer(update: Update, cx: &mut AsyncWindowContext) {
    let title = format!("Corner Table {} is available", update.version);
    let Some(path) = installed_path() else {
        offer_download(
            &title,
            "Download the latest installer from GitHub Releases.",
            cx,
        )
        .await;
        return;
    };
    let detail = if cfg!(windows) {
        "Corner Table will close and open the installer."
    } else {
        "Corner Table will download the update and restart."
    };
    if cx
        .prompt(
            PromptLevel::Info,
            &title,
            Some(detail),
            &["Install Update", "Later"],
        )
        .await
        != Ok(0)
    {
        return;
    }
    let result = cx
        .background_executor()
        .spawn(async move {
            update.download_and_install()?;
            restart(path)
        })
        .await;
    match result {
        Ok(()) => {
            let _ = cx.update(|_, cx| cx.quit());
        }
        Err(error) => offer_download("Couldn’t install the update", &error.to_string(), cx).await,
    }
}

async fn offer_download(title: &str, detail: &str, cx: &mut AsyncWindowContext) {
    if cx
        .prompt(
            PromptLevel::Info,
            title,
            Some(detail),
            &["Open Downloads", "Close"],
        )
        .await
        == Ok(0)
    {
        let _ = cx.update(|_, cx| cx.open_url(RELEASES));
    }
}

fn restart(path: PathBuf) -> Result<()> {
    if cfg!(windows) {
        // The installer takes over after the updater closes the running process.
        return Ok(());
    }
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args([
            "-c",
            r#"
        while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
        if [ "$3" = macos ]; then exec /usr/bin/open "$2"; fi
        exec "$2"
    "#,
            "sh",
        ])
        .arg(std::process::id().to_string())
        .arg(path)
        .arg(std::env::consts::OS)
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("LD_LIBRARY_PATH")
        .spawn()
        .context("restart Corner Table")?;
    Ok(())
}
