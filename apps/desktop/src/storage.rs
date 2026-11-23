use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

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
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut writer = BufWriter::new(file.as_file_mut());
        serde_json::to_writer(&mut writer, value)?;
        writer.flush()?;
    }
    file.persist(path)?;
    Ok(())
}
