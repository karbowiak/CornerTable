use crate::Color::{self, Coral, Gold, Jade, Violet};

pub const PUZZLE_COUNT: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct Puzzle {
    pub name: &'static str,
    pub hint: &'static str,
    /// Each shot that bounces and pops a group counts once.
    pub bank_target: usize,
    pub(crate) rack: &'static [&'static str],
    pub(crate) ammo: &'static [Color],
}

impl Puzzle {
    pub fn shots(&self) -> usize {
        self.ammo.len()
    }
}

pub const PUZZLES: [Puzzle; PUZZLE_COUNT] = [
    Puzzle {
        name: "Salt Bend",
        hint: "Try the left wall to reach the gold pair.",
        bank_target: 1,
        rack: &[".......GG.", ".......JJ", "........J."],
        ammo: &[Gold],
    },
    Puzzle {
        name: "Split Current",
        hint: "Bank across the table for each cluster.",
        bank_target: 2,
        rack: &[".JJ....CC.", ".V.....G.", ".VV....GG."],
        ammo: &[Jade, Coral],
    },
    Puzzle {
        name: "Glass Pilings",
        hint: "Bank into the gold cap to drop its coral stem.",
        bank_target: 1,
        rack: &[
            ".VV.GG..JJ",
            ".V..G...J",
            "....CC..C.",
            "....C...C",
            ".....C....",
        ],
        ammo: &[Gold, Jade, Violet],
    },
    Puzzle {
        name: "Tide Gate",
        hint: "Set up a match with the lone coral bubble.",
        bank_target: 2,
        rack: &[".GG..C..JJ", ".G...V..J", ".V..VV..G.", "....V...G"],
        ammo: &[Coral, Coral, Gold, Jade],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Angle, Cell, Round, Status};

    // These routes work with the standard two-degree keyboard steps.
    // Each tuple gives angle ticks, wall bounces, and bubbles popped.
    const ROUTES: [&[(i16, usize, usize)]; PUZZLE_COUNT] = [
        &[(-352, 1, 3)],
        &[(352, 1, 3), (-352, 1, 3)],
        &[(-304, 1, 4), (96, 0, 4), (-64, 0, 4)],
        &[(-304, 1, 0), (-304, 1, 3), (352, 1, 4), (96, 0, 4)],
    ];

    #[test]
    fn every_puzzle_has_an_anchored_rack_and_a_forgiving_aimed_route() {
        for (index, route) in ROUTES.into_iter().enumerate() {
            let mut round = Round::puzzle(index).unwrap();
            let puzzle = round.puzzle_info().unwrap();
            let anchored = round.connected(Cell::in_row(0), |_| true);
            assert!(
                round
                    .bubbles()
                    .all(|(cell, _)| anchored[cell.row()][cell.column()])
            );
            assert_eq!(round.shots_left(), puzzle.shots());
            assert!(round.stage_info().is_none());

            for &(ticks, bounces, popped) in route {
                assert_eq!(ticks % 16, 0);
                let aim = round.aim_at(Angle::from_ticks(ticks).unwrap()).unwrap();
                for nearby in ticks - 4..=ticks + 4 {
                    let nearby = round.aim_at(Angle::from_ticks(nearby).unwrap()).unwrap();
                    assert_eq!(nearby.landing, aim.landing, "{}", puzzle.name);
                    assert_eq!(nearby.bounces(), bounces, "{}", puzzle.name);
                }
                let shot = round.fire_at(Angle::from_ticks(ticks).unwrap()).unwrap();
                assert_eq!((shot.bounces, shot.popped.len()), (bounces, popped));
            }

            assert_eq!(round.status(), Status::Won, "{}", puzzle.name);
            assert_eq!(round.banked_matches(), puzzle.bank_target);
            assert_eq!(round.shots_left(), 0);
        }
        assert!(Round::puzzle(PUZZLE_COUNT).is_none());
    }

    #[test]
    fn a_direct_clear_misses_the_goal_and_a_double_bank_counts_as_one_match() {
        let mut direct = Round::puzzle(0).unwrap();
        let shot = direct.fire_at(Angle::from_ticks(80).unwrap()).unwrap();
        assert_eq!(shot.bounces, 0);
        assert_eq!(direct.bubbles().count(), 0);
        assert_eq!(direct.status(), Status::BankGoalMissed);

        let mut banked = Round::puzzle(0).unwrap();
        let shot = banked.fire_at(Angle::from_ticks(496).unwrap()).unwrap();
        assert_eq!(shot.bounces, 2);
        assert_eq!(banked.banked_matches(), 1);
        assert_eq!(banked.status(), Status::Won);

        let mut missed = Round::puzzle(0).unwrap();
        missed.fire_at(Angle::default()).unwrap();
        assert_eq!(missed.status(), Status::OutOfShots);
    }

    #[test]
    fn a_swap_preserves_the_authored_ammo_for_the_remaining_shots() {
        let mut round = Round::puzzle(1).unwrap();
        assert_eq!((round.current(), round.next()), (Jade, Coral));
        assert!(round.swap());
        assert!(!round.swap());
        assert_eq!(round.shots_left(), 2);
        round.fire_at(Angle::from_ticks(-352).unwrap()).unwrap();
        assert_eq!(round.current(), Jade);
        assert_eq!(round.shots_left(), 1);
        assert!(!round.can_swap());
        round.fire_at(Angle::from_ticks(352).unwrap()).unwrap();
        assert_eq!(round.status(), Status::Won);
    }
}
