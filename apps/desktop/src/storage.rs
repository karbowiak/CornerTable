use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use corner_table_api::GameId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::games::GameLaunch;
use crate::theme::Appearance;

const FORMAT: u32 = 1;

fn data_dir() -> Option<PathBuf> {
    env::var_os("CORNER_TABLE_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|dir| dir.join("dk.karbowiak.CornerTable")))
}

pub struct Session {
    path: Option<PathBuf>,
    launch: GameLaunch,
}

#[derive(Deserialize, Serialize)]
struct Envelope<Key, State> {
    format: u32,
    key: Key,
    state: State,
}

/// Keep this ID in the game's save for the lifetime of an attempt. A replay
/// starts a new attempt, even if it uses the same seed.
#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AttemptId {
    started: u128,
    process: u32,
    sequence: u64,
}

impl Default for AttemptId {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self {
            started: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            process: std::process::id(),
            sequence: NEXT.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Won,
    Lost,
    Finished,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultMetric {
    Moves(u64),
    Pairs(u64),
    Points(u64),
    TimeMillis(u64),
}

#[derive(Deserialize, Serialize)]
pub struct GameResult {
    pub game: GameId,
    #[serde(default)]
    pub launch: Option<GameLaunch>,
    pub mode: String,
    pub outcome: Outcome,
    pub metric: ResultMetric,
    pub completed_at_ms: i64,
}

impl Session {
    pub fn new(launch: &GameLaunch) -> Self {
        let path = data_dir().map(|dir| {
            let serialized = serde_json::to_vec(launch).expect("game launch consists of strings");
            let key = serialized
                .iter()
                .fold(0xcbf29ce484222325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                });
            dir.join("sessions").join(format!("{key:016x}.json"))
        });
        Self {
            path,
            launch: launch.clone(),
        }
    }

    pub fn load<State: DeserializeOwned>(&self) -> Option<State> {
        let saved: Envelope<GameLaunch, State> = read(self.path.as_deref()?)?;
        (saved.format == FORMAT && saved.key == self.launch).then_some(saved.state)
    }

    pub fn save<State: Serialize>(&self, state: &State) {
        if let Some(path) = self.path.as_deref() {
            write(
                path,
                &Envelope {
                    format: FORMAT,
                    key: &self.launch,
                    state,
                },
            );
        }
    }

    /// Record the terminal outcome from a play action. Repeated writes for
    /// the same attempt keep its original result and completion time.
    pub fn record_result(
        &self,
        attempt: AttemptId,
        mode: impl Into<String>,
        outcome: Outcome,
        metric: ResultMetric,
    ) {
        if let Some(dir) = data_dir()
            && let Err(error) = save_result(
                &dir.join("results"),
                attempt,
                GameResult {
                    game: self.launch.game.clone(),
                    launch: Some(self.launch.clone()),
                    mode: mode.into(),
                    outcome,
                    metric,
                    completed_at_ms: Utc::now().timestamp_millis(),
                },
            )
        {
            eprintln!("Unable to save Corner Table result: {error:#}");
        }
    }
}

pub fn recent_results(limit: usize) -> Vec<GameResult> {
    data_dir()
        .map(|dir| load_results(&dir.join("results"), limit))
        .unwrap_or_default()
}

fn save_result(dir: &Path, attempt: AttemptId, result: GameResult) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!(
        "{:x}-{:x}-{:x}.json",
        attempt.started, attempt.process, attempt.sequence
    ));
    let file = write_temp(
        dir,
        &Envelope {
            format: FORMAT,
            key: attempt,
            state: result,
        },
    )?;
    file.as_file().sync_all()?;

    // Independent files avoid read/modify/write races across processes.
    match file.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn load_results(dir: &Path, limit: usize) -> Vec<GameResult> {
    let Ok(files) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut results: Vec<Envelope<AttemptId, GameResult>> = files
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| read(&path))
        .filter(|result: &Envelope<_, _>| result.format == FORMAT)
        .collect();
    results.sort_unstable_by_key(|result| {
        std::cmp::Reverse((result.state.completed_at_ms, result.key))
    });
    results.truncate(limit);
    results.into_iter().map(|result| result.state).collect()
}

pub fn recent_game() -> Option<GameLaunch> {
    let saved: Envelope<(), GameLaunch> = read(&data_dir()?.join("recent.json"))?;
    (saved.format == FORMAT).then_some(saved.state)
}

pub fn remember_game(launch: &GameLaunch) {
    if let Some(dir) = data_dir() {
        write(
            &dir.join("recent.json"),
            &Envelope {
                format: FORMAT,
                key: (),
                state: launch,
            },
        );
    }
}

pub fn appearance() -> Appearance {
    data_dir()
        .and_then(|dir| read(&dir.join("appearance.json")))
        .unwrap_or_default()
}

pub fn set_appearance(appearance: Appearance) {
    if let Some(dir) = data_dir() {
        write(&dir.join("appearance.json"), &appearance);
    }
}

pub fn sound_enabled() -> bool {
    data_dir()
        .and_then(|dir| read(&dir.join("sound.json")))
        .unwrap_or(false)
}

pub fn set_sound_enabled(enabled: bool) {
    if let Some(dir) = data_dir() {
        write(&dir.join("sound.json"), &enabled);
    }
}

pub fn favorites() -> Vec<GameId> {
    data_dir()
        .and_then(|dir| read(&dir.join("favorites.json")))
        .unwrap_or_default()
}

pub fn save_favorites(favorites: &[GameId]) {
    if let Some(dir) = data_dir() {
        write(&dir.join("favorites.json"), &favorites);
    }
}

fn read<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let file = File::open(path).ok()?;
    serde_json::from_reader(BufReader::new(file)).ok()
}

fn write(path: &Path, value: &impl Serialize) {
    if let Err(error) = write_atomically(path, value) {
        eprintln!("Unable to save Corner Table progress: {error:#}");
    }
}

fn write_atomically(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path.parent().expect("save path has a parent");
    fs::create_dir_all(parent)?;
    write_temp(parent, value)?.persist(path)?;
    Ok(())
}

fn write_temp(parent: &Path, value: &impl Serialize) -> anyhow::Result<tempfile::NamedTempFile> {
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer = BufWriter::new(file.as_file_mut());
    serde_json::to_writer(&mut writer, value)?;
    writer.flush()?;
    drop(writer);
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(game: &str, completed_at_ms: i64) -> GameResult {
        GameResult {
            game: GameId(game.into()),
            launch: None,
            mode: "Classic".into(),
            outcome: Outcome::Won,
            metric: ResultMetric::Moves(12),
            completed_at_ms,
        }
    }

    #[test]
    fn retry_keeps_the_first_result_but_same_seed_replay_can_save_again() {
        let dir = tempfile::tempdir().unwrap();
        let attempt = AttemptId::default();
        let game = "solitaire";
        save_result(dir.path(), attempt, result(game, 100)).unwrap();

        let resumed: AttemptId =
            serde_json::from_slice(&serde_json::to_vec(&attempt).unwrap()).unwrap();
        save_result(dir.path(), resumed, result(game, 200)).unwrap();
        save_result(dir.path(), AttemptId::default(), result(game, 300)).unwrap();

        let results = load_results(dir.path(), 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].completed_at_ms, 300);
        assert_eq!(results[1].completed_at_ms, 100);
    }

    #[test]
    fn parallel_results_survive_a_damaged_file_and_the_recent_limit() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("results");
        assert!(load_results(&dir, 5).is_empty());
        std::thread::scope(|scope| {
            for (offset, game) in ["solitaire", "mahjong", "match-three", "gem-cascade"]
                .into_iter()
                .enumerate()
            {
                let dir = &dir;
                scope.spawn(move || {
                    for index in 0..10 {
                        save_result(
                            dir,
                            AttemptId::default(),
                            result(game, 100 + (offset as i64 * 10) + index),
                        )
                        .unwrap();
                    }
                });
            }
        });
        fs::write(dir.join("damaged.json"), "{").unwrap();
        assert_eq!(load_results(&dir, 100).len(), 40);
        let recent = load_results(&dir, 2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].game.0, "gem-cascade");
        assert_eq!(recent[0].completed_at_ms, 139);
        assert_eq!(recent[1].completed_at_ms, 138);
    }
}
