use std::collections::HashSet;

use chrono::{Datelike as _, Days, NaiveDate, Utc};
use corner_table_api::GameId;

use crate::games::{GameEntry, GameLaunch};
use crate::storage::{GameResult, Outcome};

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

pub fn completed_days(results: &[GameResult]) -> HashSet<NaiveDate> {
    results
        .iter()
        .filter(|result| result.outcome == Outcome::Won)
        .filter_map(|result| from_launch(result.launch.as_ref()?))
        .map(|puzzle| puzzle.date)
        .collect()
}

pub fn current_streak(today: NaiveDate, completed: &HashSet<NaiveDate>) -> usize {
    let start = if completed.contains(&today) { 0 } else { 1 };
    (start..)
        .map_while(|offset| today.checked_sub_days(Days::new(offset)))
        .take_while(|date| completed.contains(date))
        .count()
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

    #[test]
    fn yesterday_keeps_a_streak_alive_until_the_end_of_today() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let mut completed = HashSet::from([
            today - Days::new(1),
            today - Days::new(2),
            today - Days::new(4),
        ]);
        assert_eq!(current_streak(today, &completed), 2);
        completed.insert(today);
        assert_eq!(current_streak(today, &completed), 3);
        assert_eq!(current_streak(today + Days::new(2), &completed), 0);
    }
}
