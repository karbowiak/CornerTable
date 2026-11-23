use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::thread::{self, Thread};

use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2_avf_audio::AVAudioPlayer;
use objc2_foundation::NSData;

use super::Effect;

macro_rules! bundled {
    ($name:literal) => {
        include_bytes!(concat!("../../assets/sounds/", $name, ".wav"))
    };
}

macro_rules! local_or_bundled {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".wav"))
    };
}

// In ascending priority, matching the Effect variants. build.rs selects
// a local override or its bundled fallback for each optional cue.
const CLIPS: [&[u8]; Effect::Complete as usize + 1] = [
    local_or_bundled!("move"),
    bundled!("card-place"),
    bundled!("card-slide"),
    bundled!("ceramic-tap"),
    bundled!("wood-tap"),
    bundled!("reveal"),
    local_or_bundled!("flag"),
    local_or_bundled!("unflag"),
    bundled!("bubble-land"),
    local_or_bundled!("invalid-move"),
    bundled!("clear"),
    bundled!("card-collect"),
    bundled!("ceramic-pair"),
    bundled!("wood-pair"),
    local_or_bundled!("bubble-pop"),
    local_or_bundled!("bubble-chain"),
    local_or_bundled!("gem-clear"),
    local_or_bundled!("gem-chain"),
    local_or_bundled!("gem-chain-long"),
    bundled!("complete"),
];
const VOLUME: f32 = 0.35;

const IDLE: u8 = 0;
const STOP: u8 = u8::MAX;

pub(super) struct AudioWorker {
    pending: Arc<AtomicU8>,
    thread: Thread,
}

impl AudioWorker {
    pub(super) fn start() -> Option<Self> {
        let pending = Arc::new(AtomicU8::new(IDLE));
        let requests = Arc::clone(&pending);
        let worker = thread::Builder::new()
            .name("corner-table-audio".into())
            .spawn(move || run(&requests))
            .ok()?;
        Some(Self {
            pending,
            thread: worker.thread().clone(),
        })
    }

    pub(super) fn play(&self, effect: Effect) {
        // Keep one pending request so input bursts cannot build a playback backlog.
        self.pending.fetch_max(effect as u8 + 1, Relaxed);
        self.thread.unpark();
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        self.pending.store(STOP, Relaxed);
        self.thread.unpark();
    }
}

fn run(requests: &AtomicU8) {
    if requests.load(Relaxed) == STOP {
        return;
    }
    let mut playback = autoreleasepool(|_| Playback::new(VOLUME));
    loop {
        match requests.swap(IDLE, Relaxed) {
            IDLE => thread::park(),
            STOP => break,
            request => {
                if requests.load(Relaxed) != STOP {
                    autoreleasepool(|_| playback.play(usize::from(request - 1)));
                }
            }
        }
    }
    autoreleasepool(|_| {
        playback.stop();
        drop(playback);
    });
}

struct Playback {
    clips: [Option<Retained<AVAudioPlayer>>; CLIPS.len()],
    active: Option<usize>,
}

impl Playback {
    fn new(volume: f32) -> Self {
        let clips = CLIPS.map(|bytes| {
            let data = NSData::with_bytes(bytes);
            // SAFETY: The input is embedded PCM and each player stays on this thread.
            unsafe {
                let player =
                    AVAudioPlayer::initWithData_error(AVAudioPlayer::alloc(), &data).ok()?;
                player.setVolume(volume);
                Some(player)
            }
        });
        Self {
            clips,
            active: None,
        }
    }

    fn play(&mut self, index: usize) {
        let Some(player) = self.clips[index].as_deref() else {
            return;
        };

        // SAFETY: All calls and player creation run on the same thread. Players
        // have no delegates, and every clip starts at the beginning of its buffer.
        unsafe {
            if let Some(active) = self.active
                && let Some(playing) = self.clips[active].as_deref()
                && playing.isPlaying()
            {
                if active >= index {
                    return;
                }
                playing.stop();
            }
            player.setCurrentTime(0.0);
            self.active = player.play().then_some(index);
        }
    }

    fn stop(&mut self) {
        if let Some(index) = self.active.take()
            && let Some(player) = self.clips[index].as_deref()
        {
            // SAFETY: The worker owns the player and stops it on the same thread.
            unsafe { player.stop() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_effects_decode_as_short_one_shots() {
        autoreleasepool(|_| {
            let playback = Playback::new(VOLUME);
            for (index, player) in playback.clips.iter().enumerate() {
                let player = player
                    .as_ref()
                    .unwrap_or_else(|| panic!("Could not decode audio cue {index}"));
                // SAFETY: The player was created here and never leaves this thread.
                let duration = unsafe { player.duration() };
                assert!((0.01..0.75).contains(&duration), "Audio cue {index}");
            }
        });
    }
}
