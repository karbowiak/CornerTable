use corner_table_chess::{Decision, Game, Move};
use serde::{Deserialize, Serialize};

use super::super::{GameEntry, GameLaunch};
use crate::storage::Session;

const SAVE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Local,
    Computer,
}

impl Mode {
    fn slot(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Computer => 1,
        }
    }
}

pub(super) struct Table {
    session: Session,
    matches: [Game; 2],
    pub(super) mode: Mode,
    pub(super) flipped: bool,
}

impl Table {
    pub(super) fn open(launch: &GameLaunch) -> Self {
        let session = Session::new(launch);
        let saved = session
            .load::<Saved>()
            .filter(|save| save.version == SAVE_VERSION);
        let (mode, flipped, matches) = if let Some(saved) = saved {
            (
                saved.mode,
                saved.flipped,
                saved
                    .matches
                    .map(|record| record.restore().unwrap_or_default()),
            )
        } else {
            let mode = match &launch.entry {
                GameEntry::Mode(id) if id == "computer" => Mode::Computer,
                _ => Mode::Local,
            };
            (mode, false, Default::default())
        };
        Self {
            session,
            matches,
            mode,
            flipped,
        }
    }

    pub(super) fn game(&self) -> &Game {
        &self.matches[self.mode.slot()]
    }

    pub(super) fn game_mut(&mut self) -> &mut Game {
        &mut self.matches[self.mode.slot()]
    }

    pub(super) fn save(&self) {
        self.session.save(&Saved {
            version: SAVE_VERSION,
            mode: self.mode,
            flipped: self.flipped,
            matches: self.matches.each_ref().map(MatchRecord::capture),
        });
    }
}

#[derive(Deserialize, Serialize)]
struct Saved {
    version: u8,
    mode: Mode,
    flipped: bool,
    matches: [MatchRecord; 2],
}

#[derive(Deserialize, Serialize)]
struct MatchRecord {
    moves: Vec<String>,
    decision: Option<DecisionRecord>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
enum DecisionRecord {
    ClaimDraw { intended: Option<String> },
    AgreeDraw,
    Resign,
}

impl MatchRecord {
    fn capture(game: &Game) -> Self {
        let decision = game.decision().map(|decision| match decision {
            Decision::ClaimDraw { intended } => DecisionRecord::ClaimDraw {
                intended: intended.map(|movement| movement.to_string()),
            },
            Decision::AgreeDraw => DecisionRecord::AgreeDraw,
            Decision::Resign => DecisionRecord::Resign,
        });
        Self {
            moves: game.moves().map(|play| play.movement.to_string()).collect(),
            decision,
        }
    }

    fn restore(self) -> Option<Game> {
        // The 75-move rule bounds standard games below this count, including
        // every possible pawn move and capture that resets the clock.
        if self.moves.len() > 20_000 {
            return None;
        }
        let mut game = Game::default();
        for movement in self.moves {
            game.play(parse_move(&movement)?)?;
        }
        if let Some(decision) = self.decision {
            let decision = match decision {
                DecisionRecord::ClaimDraw { intended } => Decision::ClaimDraw {
                    intended: match intended {
                        Some(movement) => Some(parse_move(&movement)?),
                        None => None,
                    },
                },
                DecisionRecord::AgreeDraw => Decision::AgreeDraw,
                DecisionRecord::Resign => Decision::Resign,
            };
            if !game.decide(decision) {
                return None;
            }
        }
        Some(game)
    }
}

fn parse_move(text: &str) -> Option<Move> {
    if !matches!(text.len(), 4 | 5) {
        return None;
    }
    let movement: Move = text.parse().ok()?;
    (movement.to_string() == text).then_some(movement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use corner_table_chess::{Color, DrawClaim, Ending, Piece, Square};

    #[test]
    fn replay_preserves_board_claim_and_undo() {
        let mut game = Game::default();
        let cycle = ["g1f3", "g8f6", "f3g1", "f6g8"];
        for movement in cycle.into_iter().cycle().take(7) {
            game.play(movement.parse().unwrap()).unwrap();
        }
        assert!(game.decide(Decision::ClaimDraw {
            intended: Some("f6g8".parse().unwrap()),
        }));
        let json = serde_json::to_string(&MatchRecord::capture(&game)).unwrap();
        let record: MatchRecord = serde_json::from_str(&json).unwrap();
        let mut restored = record.restore().unwrap();
        assert_eq!(
            restored.ending(),
            Some(Ending::ClaimedDraw(DrawClaim::ThreefoldRepetition))
        );
        assert_eq!(
            restored.piece(Square::F6),
            Some((Color::Black, Piece::Knight))
        );
        assert!(restored.undo(None));
        assert_eq!(restored.moves().len(), 7);
        assert!(restored.undo(None));
        assert_eq!(restored.moves().len(), 6);
        assert_eq!(restored.moves().next_back().unwrap().san, "Nf6");
    }

    #[test]
    fn invalid_moves_and_false_endings_are_rejected() {
        for movement in ["e2e4x", "e2e4k", "e2e4qq", "e7e5", "e2é4"] {
            assert!(
                MatchRecord {
                    moves: vec![movement.into()],
                    decision: None
                }
                .restore()
                .is_none()
            );
        }
        assert!(
            MatchRecord {
                moves: vec!["e2e4".into()],
                decision: Some(DecisionRecord::ClaimDraw { intended: None }),
            }
            .restore()
            .is_none()
        );
    }
}
