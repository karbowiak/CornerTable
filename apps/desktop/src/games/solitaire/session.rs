use corner_table_solitaire::{DealMode, Game, Move, Place, Variant};
use serde::{Deserialize, Serialize};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::{AttemptId, Outcome, ResultMetric, Session},
};

use super::mode_label;

// Increment when a deal generator or move encoding changes incompatibly.
const REPLAY_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct SavedTable {
    seed: u64,
    mode: String,
}

pub(super) struct TableSession {
    session: Session,
    seed: u64,
}

impl TableSession {
    pub(super) fn open(launch: &GameLaunch, variant: Variant) -> (Self, Deal, Vec<Deal>) {
        let session = Session::new(launch);
        let saved = session.load::<SavedTable>();
        let seed = saved
            .as_ref()
            .map_or_else(|| launch.seed(), |saved| saved.seed);
        let modes = variant.deal_modes();
        let selected = saved
            .as_ref()
            .and_then(|saved| modes.iter().position(|&mode| mode_id(mode) == saved.mode))
            .unwrap_or(0);
        let mut deals: Vec<_> = modes
            .iter()
            .map(|&mode| Deal::open(launch, mode, seed))
            .collect();
        let deal = deals.swap_remove(selected);
        let table = Self { session, seed };
        if saved.is_none() {
            table.save_mode(deal.game.mode());
        }
        (table, deal, deals)
    }

    pub(super) fn save_mode(&self, mode: DealMode) {
        self.session.save(&SavedTable {
            seed: self.seed,
            mode: mode_id(mode).into(),
        });
    }
}

pub(super) struct Deal {
    session: Session,
    saved: SavedDeal,
    pub(super) game: Game,
    pub(super) selected: Option<Place>,
    pub(super) status: &'static str,
}

#[derive(Deserialize, Serialize)]
struct SavedDeal {
    version: u32,
    seed: u64,
    turns: Vec<Turn>,
    #[serde(default)]
    attempt: AttemptId,
    #[serde(default)]
    best_moves: Option<usize>,
}

impl SavedDeal {
    fn new(seed: u64) -> Self {
        Self {
            version: REPLAY_VERSION,
            seed,
            turns: Vec::new(),
            attempt: AttemptId::default(),
            best_moves: None,
        }
    }

    fn replay(&self, mode: DealMode) -> Option<Game> {
        if self.version != REPLAY_VERSION {
            return None;
        }
        Game::replay(mode, self.seed, self.turns.iter().map(|turn| turn.0))
    }

    fn record_win(&mut self, game: &Game) {
        if game.won() && self.best_moves.is_none_or(|best| game.moves < best) {
            self.best_moves = Some(game.moves);
        }
    }
}

impl Deal {
    fn open(launch: &GameLaunch, mode: DealMode, seed: u64) -> Self {
        // A broken replay affects only its own deal style. Launch kind and ID
        // keep collection deals separate from challenge or classic entries.
        let entry = match &launch.entry {
            GameEntry::Classic => String::from("classic"),
            GameEntry::Mode(id) => format!("mode/{id}"),
            GameEntry::Challenge(id) => format!("challenge/{id}"),
        };
        let session = Session::new(&GameLaunch {
            game: launch.game.clone(),
            entry: GameEntry::Mode(format!("solitaire-deal/{entry}/{}", mode_id(mode))),
        });
        let (game, mut saved) = session
            .load::<SavedDeal>()
            .and_then(|saved| Some((saved.replay(mode)?, saved)))
            .unwrap_or_else(|| (Game::with_mode(mode, seed), SavedDeal::new(seed)));
        saved.best_moves = saved.best_moves.filter(|&moves| moves > 0);
        saved.record_win(&game);
        Self {
            session,
            saved,
            game,
            selected: None,
            status: "",
        }
    }

    pub(super) fn best_moves(&self) -> Option<usize> {
        self.saved.best_moves
    }

    pub(super) fn restart(&mut self, new_seed: Option<u64>) {
        let mode = self.game.mode();
        let seed = match (new_seed, mode) {
            // Walk the catalog from the opening seed so New deal shows a
            // different shuffle on each visit until the catalog wraps.
            (Some(_), DealMode::KlondikeDrawOneVerified) => self.saved.seed.wrapping_add(1),
            (seed, _) => seed.unwrap_or(self.saved.seed),
        };
        self.game = Game::with_mode(mode, seed);
        self.saved.seed = seed;
        self.saved.turns.clear();
        self.saved.attempt = AttemptId::default();
        self.save();
    }

    pub(super) fn apply(&mut self, action: Move) -> bool {
        if !self.game.play(action) {
            return false;
        }
        self.saved.turns.push(Turn(action));
        self.saved.record_win(&self.game);
        if self.game.won() {
            let mode = self.game.mode();
            let variant = self.game.variant;
            let label = if variant.deal_modes().len() == 1 {
                variant.title().to_owned()
            } else {
                format!("{} · {}", variant.title(), mode_label(mode))
            };
            self.session.record_result(
                self.saved.attempt,
                label,
                Outcome::Won,
                ResultMetric::Moves(self.game.moves as u64),
            );
        }
        self.save();
        true
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.saved.turns.is_empty()
    }

    pub(super) fn undo(&mut self) -> bool {
        if self.saved.turns.pop().is_none() {
            return false;
        }
        // These moves were accepted by the same rules, either during play or on load.
        self.game = self
            .saved
            .replay(self.game.mode())
            .expect("accepted moves replay from their deal seed");
        self.save();
        true
    }

    fn save(&mut self) {
        self.selected = None;
        self.status = "";
        self.session.save(&self.saved);
    }
}

fn mode_id(mode: DealMode) -> &'static str {
    match mode {
        DealMode::KlondikeDrawOne => "draw-one",
        DealMode::KlondikeDrawThree => "draw-three",
        DealMode::KlondikeDrawOneVerified => "draw-one-verified",
        DealMode::SpiderOneSuit => "one-suit",
        DealMode::SpiderTwoSuits => "two-suits",
        DealMode::SpiderFourSuits => "four-suits",
        DealMode::FreeCell => "freecell",
        DealMode::Pyramid => "pyramid",
        DealMode::TriPeaks => "tripeaks",
        DealMode::TriPeaksSolvable => "tripeaks-solvable",
    }
}

// Keep the on-disk encoding in the desktop, independent of the rules crate.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct Turn(#[serde(with = "SavedMove")] Move);

#[derive(Deserialize, Serialize)]
#[serde(remote = "Move", rename_all = "kebab-case")]
enum SavedMove {
    Draw,
    Transfer {
        #[serde(with = "SavedPlace")]
        from: Place,
        #[serde(with = "SavedPlace")]
        to: Place,
    },
    Clear(#[serde(with = "SavedPlace")] Place),
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Place", rename_all = "kebab-case")]
enum SavedPlace {
    Stock,
    Waste,
    Tableau(usize, usize),
    Foundation(usize),
    Cell(usize),
    Layout(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_styles_restore_the_board_and_its_undo_prefix() {
        let seed = u64::MAX;
        for variant in [
            Variant::Klondike,
            Variant::Spider,
            Variant::FreeCell,
            Variant::Pyramid,
            Variant::TriPeaks,
        ] {
            for &mode in variant.deal_modes() {
                let turns = if variant == Variant::FreeCell {
                    [6, 5].map(|index| Move::Transfer {
                        from: Place::Tableau(0, index),
                        to: Place::Cell(6 - index),
                    })
                } else {
                    [Move::Draw; 2]
                };
                let mut game = Game::with_mode(mode, seed);
                assert!(game.play(turns[0]));
                let after_one = game.clone();
                assert!(game.play(turns[1]));

                let mut saved = SavedDeal::new(seed);
                saved.turns.extend(turns.map(Turn));
                let data = serde_json::to_vec(&saved).unwrap();
                let mut restored: SavedDeal = serde_json::from_slice(&data).unwrap();
                assert_eq!(restored.replay(mode).unwrap(), game);
                restored.turns.pop();
                assert_eq!(restored.replay(mode).unwrap(), after_one);
                restored.turns.pop();
                assert_eq!(restored.replay(mode).unwrap(), Game::with_mode(mode, seed));
            }
        }
    }

    #[test]
    fn corrupt_and_unsupported_replays_are_rejected() {
        let mode = DealMode::TriPeaks;
        let mut saved = SavedDeal::new(18);
        saved.version -= 1;
        assert!(saved.replay(mode).is_none());
        saved.version = REPLAY_VERSION;
        saved.turns.push(Turn(Move::Draw));
        saved
            .turns
            .push(Turn(Move::Clear(Place::Layout(usize::MAX))));
        assert!(saved.replay(mode).is_none());
        assert!(
            serde_json::from_str::<SavedDeal>(r#"{"version":1,"seed":18,"turns":["unknown"]}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<SavedDeal>(r#"{"version":1,"seed":18,"turns":[]}"#)
                .unwrap()
                .best_moves
                .is_none()
        );
    }
}
