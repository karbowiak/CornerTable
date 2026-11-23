use corner_table_memory_match::{Board, Checkpoint, Layout};
use serde::{Deserialize, Serialize};

use crate::storage::{AttemptId, Outcome, ResultMetric, Session};

pub(super) struct Round {
    pub board: Board,
    pub best: Option<u64>,
    attempt: AttemptId,
}

#[derive(Deserialize, Serialize)]
pub(super) struct Saved {
    pub layout: Layout,
    pub hold_mismatches: bool,
    pub rounds: [SavedRound; 3],
}

#[derive(Deserialize, Serialize)]
pub(super) struct SavedRound {
    board: Checkpoint,
    best: Option<u64>,
    attempt: AttemptId,
}

impl Round {
    pub fn deal(layout: Layout, seed: u64, best: Option<u64>) -> Self {
        Self {
            board: Board::new(layout, seed),
            best,
            attempt: AttemptId::default(),
        }
    }

    pub fn capture(&self) -> SavedRound {
        SavedRound {
            board: self.board.checkpoint(),
            best: self.best,
            attempt: self.attempt,
        }
    }

    pub fn complete(&mut self) {
        let score = self.board.attempts();
        self.best = Some(self.best.map_or(score, |best| best.min(score)));
    }

    pub fn record_result(&self, session: &Session) {
        if self.board.is_complete() {
            let layout = self.board.layout();
            session.record_result(
                self.attempt,
                format!("{} cards", layout.card_count()),
                Outcome::Won,
                ResultMetric::Moves(self.board.attempts()),
            );
        }
    }
}

impl SavedRound {
    pub fn restore(self, layout: Layout, seed: u64) -> Round {
        if self.board.layout != layout {
            return Round::deal(layout, seed, None);
        }
        let best = self.best.filter(|&best| best >= layout.pair_count() as u64);
        let Some(board) = Board::restore(self.board) else {
            return Round::deal(layout, seed, best);
        };
        let mut round = Round {
            board,
            best,
            attempt: self.attempt,
        };
        if round.board.is_complete() {
            round.complete();
        }
        round
    }
}
