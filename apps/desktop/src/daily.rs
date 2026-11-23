use chrono::{Datelike as _, Days, NaiveDate, Utc};
use corner_table_api::GameId;

use crate::games::{GameEntry, GameLaunch};

pub struct DailyPuzzle {
    pub date: NaiveDate,
    pub game: &'static str,
    pub title: &'static str,
    pub objective: &'static str,
}

const ROTATION: [(&str, &str, &str); 7] = [
    (
        "mahjong",
        "Mahjong",
        "Clear the board. For a Clean Clear, finish without hints or undo.",
    ),
    (
        "minesweeper",
        "Minesweeper",
        "Reveal every safe square in the field.",
    ),
    (
        "sudoku",
        "Sudoku",
        "Solve the grid. Can you finish without a hint?",
    ),
    (
        "mahjong-cubes",
        "Mahjong Cubes",
        "Rotate the stack and remove every matching pair.",
    ),
    (
        "mine-trails",
        "Mine Trails",
        "Collect the room diamonds and reach the vault.",
    ),
    (
        "match-three",
        "Match Three",
        "Reach the score goal before your last move.",
    ),
    (
        "bubble-shooter",
        "Bubble Shooter",
        "Clear the field before bubbles reach the bottom.",
    ),
];

impl DailyPuzzle {
    pub fn launch(&self) -> GameLaunch {
        GameLaunch {
            game: GameId(self.game.into()),
            entry: GameEntry::Challenge(format!("daily-v1-{}-{}", self.date, self.game)),
        }
    }
}

pub fn today() -> DailyPuzzle {
    puzzle_on(Utc::now().date_naive())
}

pub fn previous(today: NaiveDate) -> impl Iterator<Item = DailyPuzzle> {
    (1..=7).filter_map(move |offset| today.checked_sub_days(Days::new(offset)).map(puzzle_on))
}

pub fn from_launch(launch: &GameLaunch) -> Option<DailyPuzzle> {
    let GameEntry::Challenge(id) = &launch.entry else {
        return None;
    };
    let (date, _) = id.strip_prefix("daily-v1-")?.split_at_checked(10)?;
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let daily = puzzle_on(date);
    (daily.launch() == *launch).then_some(daily)
}

fn puzzle_on(date: NaiveDate) -> DailyPuzzle {
    let (game, title, objective) = ROTATION[date.weekday().num_days_from_monday() as usize];
    DailyPuzzle {
        date,
        game,
        title,
        objective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_week_has_seven_distinct_games_and_day_ids_round_trip() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let puzzles: Vec<_> = previous(date).collect();
        let games: std::collections::HashSet<_> =
            puzzles.iter().map(|puzzle| puzzle.game).collect();
        assert_eq!(games.len(), 7);
        for puzzle in puzzles {
            assert_eq!(from_launch(&puzzle.launch()).unwrap().date, puzzle.date);
        }
        let same_game_last_week = puzzle_on(date - Days::new(7));
        assert_eq!(same_game_last_week.game, puzzle_on(date).game);
        assert_ne!(same_game_last_week.launch(), puzzle_on(date).launch());
    }
}
