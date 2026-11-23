use std::collections::HashSet;

use chrono::{Datelike as _, Days, Months, NaiveDate, Utc};
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

pub fn month_weeks(date: NaiveDate) -> impl Iterator<Item = [Option<NaiveDate>; 7]> {
    let first = month_start(date);
    let leading = first.weekday().num_days_from_monday();
    let last_day = month_end(first).day();
    (0..(leading + last_day).div_ceil(7)).map(move |week| {
        std::array::from_fn(|column| {
            (week * 7 + column as u32)
                .checked_sub(leading)
                .and_then(|index| first.with_day(index + 1))
        })
    })
}

pub fn previous_month(date: NaiveDate) -> Option<NaiveDate> {
    month_start(date).pred_opt().filter(|day| day.year() >= 0)
}

pub fn next_month(date: NaiveDate, today: NaiveDate) -> Option<NaiveDate> {
    let first = month_start(date).checked_add_months(Months::new(1))?;
    (first <= today).then(|| month_end(first).min(today))
}

fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap()
}

fn month_end(first: NaiveDate) -> NaiveDate {
    first
        .checked_add_months(Months::new(1))
        .and_then(|next| next.pred_opt())
        .unwrap_or(NaiveDate::MAX)
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

pub fn puzzle_on(date: NaiveDate) -> DailyPuzzle {
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
        let puzzles: Vec<_> = (1..=7)
            .map(|offset| puzzle_on(date - Days::new(offset)))
            .collect();
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

    #[test]
    fn month_weeks_align_monday_through_sunday_and_pad_month_edges() {
        let february = NaiveDate::from_ymd_opt(2024, 2, 15).unwrap();
        let weeks: Vec<_> = month_weeks(february).collect();
        assert_eq!(weeks.len(), 5);
        assert_eq!(weeks[0][..3], [None; 3]);
        assert_eq!(weeks[0][3], february.with_day(1));
        assert_eq!(weeks[4][3], february.with_day(29));
        assert_eq!(weeks[4][4..], [None; 3]);

        let august = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let weeks: Vec<_> = month_weeks(august).collect();
        assert_eq!(weeks.len(), 6);
        assert_eq!(weeks[0][5], Some(august));
        assert_eq!(weeks[5][0], august.with_day(31));
        assert_eq!(weeks[5][1..], [None; 6]);
    }

    #[test]
    fn month_navigation_keeps_the_last_seven_days_across_new_year() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        let december = previous_month(today).unwrap();
        assert_eq!(december, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
        assert_eq!(next_month(december, today), Some(today));
        assert_eq!(next_month(today, today), None);

        let days: HashSet<_> = month_weeks(today)
            .chain(month_weeks(december))
            .flatten()
            .flatten()
            .collect();
        assert!((1..=7).all(|offset| days.contains(&(today - Days::new(offset)))));
    }
}
