use gpui::App;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
impl gpui::Global for macos::AudioWorker {}

/// The sound for a game action. Pick the strongest applicable effect per action.
/// Keep the order in sync with the macOS clip table, from taps to completion.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Effect {
    Move,
    CardPlace,
    CardSlide,
    CeramicTap,
    WoodTap,
    Reveal,
    Flag,
    Unflag,
    BubbleLand,
    InvalidMove,
    Clear,
    CardCollect,
    CeramicPair,
    WoodPair,
    BubblePop,
    BubbleChain,
    GemClear,
    GemChain,
    GemChainLong,
    Complete,
}

impl Effect {
    pub fn gems(waves: usize) -> Self {
        match waves {
            0 | 1 => Self::GemClear,
            2 => Self::GemChain,
            _ => Self::GemChainLong,
        }
    }
}

/// Whether sound effects are implemented on this platform.
pub const fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Apply the saved opt-in or a change to the user's sound preference.
/// Sound is disabled until this is called with `true`.
pub fn set_enabled(cx: &mut App, enabled: bool) {
    #[cfg(target_os = "macos")]
    if enabled {
        if !cx.has_global::<macos::AudioWorker>()
            && let Some(worker) = macos::AudioWorker::start()
        {
            cx.set_global(worker);
        }
    } else if cx.has_global::<macos::AudioWorker>() {
        cx.remove_global::<macos::AudioWorker>();
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (cx, enabled);
}

/// Request a sound without waiting on the audio backend. Safe when disabled.
pub fn play(cx: &App, effect: Effect) {
    #[cfg(target_os = "macos")]
    if let Some(worker) = cx.try_global::<macos::AudioWorker>() {
        worker.play(effect);
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (cx, effect);
}
