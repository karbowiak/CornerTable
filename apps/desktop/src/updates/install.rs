use std::path::Path;

use anyhow::{Context, Result, ensure};

#[cfg(target_os = "linux")]
pub(super) fn verified_package(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    ensure!(
        bytes.get(8..11) == Some(b"AI\x02"),
        "invalid AppImage header"
    );
    let parent = path.parent().context("find the AppImage directory")?;
    let permissions = path.metadata()?.permissions();
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().set_permissions(permissions)?;
    staged.as_file().sync_all()?;
    staged.persist(path)?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn verified_package(_: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::process::Command;

    ensure!(bytes.starts_with(b"MZ"), "invalid Windows setup executable");
    let mut setup = tempfile::Builder::new()
        .prefix("corner-table-setup-")
        .suffix(".exe")
        .tempfile()?;
    setup.write_all(bytes)?;
    let path = setup.into_temp_path().keep()?;
    if let Err(error) = Command::new(&path).spawn() {
        let _ = std::fs::remove_file(path);
        return Err(error).context("start Corner Table Setup");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn verified_package(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{fs, process::Command};

    let parent = path.parent().context("find the Applications directory")?;
    // Keep the new and previous bundles on the same filesystem as the installed app.
    let stage = tempfile::Builder::new()
        .prefix(".corner-table-update-")
        .tempdir_in(parent)?;
    tar::Archive::new(flate2::read::GzDecoder::new(bytes)).unpack(stage.path())?;
    let incoming = stage.path().join("Corner Table.app");
    let signature = Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict"])
        .arg(&incoming)
        .output()?;
    ensure!(
        signature.status.success(),
        "the downloaded app bundle did not pass codesign verification: {}",
        String::from_utf8_lossy(&signature.stderr).trim()
    );

    let previous = stage.path().join("previous.app");
    fs::rename(path, &previous).context("move the existing app aside")?;
    if let Err(error) = fs::rename(&incoming, path) {
        if let Err(restore_error) = fs::rename(&previous, path) {
            let backup = stage.keep().join("previous.app");
            anyhow::bail!(
                "replace the app: {error}, restore the previous app: {restore_error}. Backup: {}",
                backup.display()
            );
        }
        return Err(error).context("replace Corner Table.app");
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) fn verified_package(_: &Path, _: &[u8]) -> Result<()> {
    anyhow::bail!("This platform uses manual downloads")
}
