use std::collections::{HashMap, HashSet};
use std::error::Error;

use serde::de::DeserializeOwned;

use crate::{Board, Game, GameId, Home, Library, Period};

/// In-memory stand-in for the v1 HTTP API. The JSON ships with the desktop binary.
pub struct FixtureApi {
    pub library: Library,
    pub home: Home,
    boards: HashMap<(GameId, Period), Board>,
}

impl FixtureApi {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let library: Library = read("library", include_str!("../fixtures/v1/library.json"))?;
        let home: Home = read("home", include_str!("../fixtures/v1/home.json"))?;
        let responses: Vec<Board> = read(
            "leaderboards",
            include_str!("../fixtures/v1/leaderboards.json"),
        )?;

        let game_ids: HashSet<_> = library.games.iter().map(|game| &game.id).collect();
        if game_ids.len() != library.games.len() {
            return Err("duplicate game ID in library fixture".into());
        }
        if !game_ids.contains(&home.challenge.game_id) {
            return Err("daily challenge references a missing game".into());
        }

        let mut boards = HashMap::new();
        for board in responses {
            if !game_ids.contains(&board.game_id) {
                return Err(
                    format!("leaderboard references missing game: {}", board.game_id.0).into(),
                );
            }
            if let Some(guest) = &board.guest_standing
                && guest.player_id != home.guest.id
            {
                return Err(format!("incorrect guest on leaderboard: {}", board.game_id.0).into());
            }
            if boards
                .insert((board.game_id.clone(), board.period), board)
                .is_some()
            {
                return Err("duplicate leaderboard for game and period".into());
            }
        }

        Ok(Self {
            library,
            home,
            boards,
        })
    }

    pub fn game(&self, id: &GameId) -> Option<&Game> {
        self.library.games.iter().find(|game| &game.id == id)
    }

    pub fn leaderboard(&self, game: &GameId, period: Period) -> Option<&Board> {
        self.boards.get(&(game.clone(), period))
    }
}

fn read<T: DeserializeOwned>(name: &str, json: &str) -> Result<T, Box<dyn Error>> {
    serde_json::from_str(json)
        .map_err(|error| format!("could not read {name} fixture: {error}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_boards_keep_game_and_period_independent() {
        let api = FixtureApi::load().unwrap();
        let game = &api.home.challenge.game_id;
        let daily = api.leaderboard(game, Period::Today).unwrap();
        let weekly = api.leaderboard(game, Period::ThisWeek).unwrap();
        assert_ne!(daily.leaders[0].player_id, weekly.leaders[0].player_id);
        assert_eq!(
            daily.guest_standing.as_ref().unwrap().player_id,
            api.home.guest.id
        );
        assert!(
            api.leaderboard(&GameId("sudoku".into()), Period::Today)
                .is_none()
        );
    }
}
