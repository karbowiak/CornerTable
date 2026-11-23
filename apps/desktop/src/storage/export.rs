use std::ffi::OsStr;

use anyhow::{Context, Result, ensure};
use serde_json::value::RawValue;

use super::*;

#[derive(Serialize)]
struct Preferences {
    appearance: Option<Appearance>,
    sound: Option<bool>,
    reduce_motion: Option<bool>,
    window_size: Option<WindowSize>,
    favorites: Option<Vec<GameId>>,
    recent: Option<Envelope<(), GameLaunch>>,
}

#[derive(Serialize)]
struct Progress {
    format: &'static str,
    version: u32,
    preferences: Preferences,
    sessions: Vec<Envelope<GameLaunch, Box<RawValue>>>,
    results: Vec<Envelope<AttemptId, GameResult>>,
}

pub fn export_progress(profile: &Path, destination: &Path) -> Result<(usize, usize)> {
    let profile = match profile.canonicalize() {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            anyhow::bail!("There is no saved Corner Table profile to export yet")
        }
        path => path.context("Could not open the Corner Table profile")?,
    };
    let parent = destination
        .parent()
        .context("Choose a file destination")?
        .canonicalize()
        .context("Could not open the selected destination folder")?;
    // Directory identity also catches case and Unicode aliases on macOS volumes.
    let profile_dir = same_file::Handle::from_path(&profile)?;
    for ancestor in parent.ancestors() {
        ensure!(
            same_file::Handle::from_path(ancestor)? != profile_dir,
            "Choose a destination outside the Corner Table profile"
        );
    }
    let destination = parent.join(destination.file_name().context("Choose a file name")?);

    let preferences = Preferences {
        appearance: preference(&profile, "appearance.json")?,
        sound: preference(&profile, "sound.json")?,
        reduce_motion: preference(&profile, "reduce-motion.json")?,
        window_size: preference(&profile, "window-size.json")?,
        favorites: preference(&profile, "favorites.json")?,
        recent: preference(&profile, "recent.json")?,
    };
    if let Some(recent) = &preferences.recent {
        ensure!(recent.format == FORMAT, "Unsupported recent.json format");
    }
    let progress = Progress {
        format: "corner-table-progress",
        version: 1,
        preferences,
        sessions: records(
            &profile.join("sessions"),
            |stem| hex(stem) && stem.len() == 16,
            session_filename,
        )?,
        results: records(
            &profile.join("results"),
            |stem| {
                let mut parts = stem.split('-');
                (0..3).all(|_| parts.next().is_some_and(hex)) && parts.next().is_none()
            },
            |attempt| result_filename(*attempt),
        )?,
    };
    let file = write_temp(&parent, &progress)?;
    file.as_file().sync_all()?;
    file.persist(destination)
        .context("Could not save the progress export")?;
    Ok((progress.sessions.len(), progress.results.len()))
}

fn hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn preference<T: DeserializeOwned>(profile: &Path, name: &str) -> Result<Option<T>> {
    let path = profile.join(name);
    match path.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        metadata => {
            ensure!(metadata?.is_file(), "Expected a regular file for {name}");
            decode(&path).map(Some)
        }
    }
}

fn decode<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path).with_context(|| format!("Could not read {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("Invalid saved data in {}", path.display()))
}

fn records<K: DeserializeOwned, V: DeserializeOwned>(
    dir: &Path,
    matches_stem: impl Fn(&str) -> bool,
    filename: impl Fn(&K) -> String,
) -> Result<Vec<Envelope<K, V>>> {
    match dir.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        metadata => ensure!(
            metadata?.is_dir(),
            "Expected a directory at {}",
            dir.display()
        ),
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() == Some(OsStr::new("json"))
            && path
                .file_stem()
                .and_then(OsStr::to_str)
                .is_some_and(&matches_stem)
        {
            ensure!(
                entry.file_type()?.is_file(),
                "Expected a regular file at {}",
                path.display()
            );
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let record: Envelope<K, V> = decode(&path)?;
            ensure!(
                record.format == FORMAT
                    && path.file_name() == Some(OsStr::new(&filename(&record.key))),
                "Unexpected save format or key in {}",
                path.display()
            );
            Ok(record)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GameEntry;

    #[test]
    fn progress_export_preserves_records_and_exact_session_numbers() {
        let profile = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let file = dest.path().join("backup.json");
        let profile = profile.path();
        let launch = GameLaunch {
            game: GameId("solitaire".into()),
            entry: GameEntry::Classic,
        };
        let session_path = profile.join("sessions").join(session_filename(&launch));
        write_atomically(
            &session_path,
            &Envelope {
                format: FORMAT,
                key: &launch,
                state: u128::MAX,
            },
        )
        .unwrap();
        let attempt = AttemptId::default();
        save_result(
            &profile.join("results"),
            attempt,
            GameResult {
                game: launch.game.clone(),
                launch: None,
                mode: "Classic".into(),
                outcome: Outcome::Won,
                metric: ResultMetric::Moves(1),
                completed_at_ms: 7,
            },
        )
        .unwrap();
        fs::write(profile.join("sound.json"), "true").unwrap();
        fs::write(profile.join("favorites.json"), r#"["solitaire"]"#).unwrap();
        fs::write(profile.join("other.json"), "private").unwrap();
        fs::write(profile.join("sessions/notes.json"), "private").unwrap();
        fs::create_dir(profile.join("jigsaw-photos")).unwrap();
        fs::write(profile.join("jigsaw-photos/mine.jpg"), "private").unwrap();
        fs::write(&file, "old backup").unwrap();
        assert_eq!(export_progress(profile, &file).unwrap(), (1, 1));

        #[derive(Deserialize)]
        struct Saved {
            format: String,
            version: u32,
            preferences: serde_json::Value,
            sessions: Vec<Envelope<GameLaunch, u128>>,
            results: Vec<Envelope<AttemptId, GameResult>>,
        }
        let backup: Saved = decode(&file).unwrap();
        assert_eq!(backup.format, "corner-table-progress");
        assert_eq!(backup.version, 1);
        assert_eq!(backup.preferences["sound"], true);
        assert_eq!(backup.preferences["favorites"][0], "solitaire");
        assert!(backup.preferences["appearance"].is_null());
        assert_eq!(backup.sessions[0].state, u128::MAX);
        assert_eq!(backup.sessions[0].key, launch);
        assert!(backup.results[0].key == attempt);
        assert_eq!(backup.results[0].state.completed_at_ms, 7);
        assert!(!fs::read_to_string(&file).unwrap().contains("private"));
    }

    #[test]
    fn progress_export_keeps_existing_files_on_failure() {
        let profile = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let file = dest.path().join("backup.json");
        fs::write(&file, "last good backup").unwrap();
        let preference = profile.path().join("sound.json");
        fs::write(&preference, "true").unwrap();
        assert!(export_progress(profile.path(), &preference).is_err());
        assert_eq!(fs::read_to_string(&preference).unwrap(), "true");
        fs::create_dir(profile.path().join("sessions")).unwrap();
        fs::write(profile.path().join("sessions/0000000000000000.json"), "{").unwrap();
        assert!(export_progress(profile.path(), &file).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "last good backup");
        assert_eq!(dest.path().read_dir().unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn progress_export_rejects_linked_inputs_and_profile_destination_aliases() {
        use std::os::unix::fs::symlink;

        let profile = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let alias = dest.path().join("alias");
        symlink(profile.path(), &alias).unwrap();
        assert!(export_progress(profile.path(), &alias.join("backup.json")).is_err());
        assert!(!profile.path().join("backup.json").exists());
        let private = dest.path().join("private");
        fs::write(&private, "true").unwrap();
        symlink(private, profile.path().join("sound.json")).unwrap();
        let backup = dest.path().join("backup.json");
        assert!(export_progress(profile.path(), &backup).is_err());
        assert!(!backup.exists());
    }
}
