pub fn court_portrait(suit: &str, rank: u8) -> Option<&'static str> {
    let portraits = match suit {
        "♠" => [
            "games/solitaire/court-jack.webp",
            "games/solitaire/court-queen.webp",
            "games/solitaire/court-king.webp",
        ],
        "♥" => [
            "games/solitaire/court-hearts-jack.webp",
            "games/solitaire/court-hearts-queen.webp",
            "games/solitaire/court-hearts-king.webp",
        ],
        "♦" => [
            "games/solitaire/court-diamonds-jack.webp",
            "games/solitaire/court-diamonds-queen.webp",
            "games/solitaire/court-diamonds-king.webp",
        ],
        "♣" => [
            "games/solitaire/court-clubs-jack.webp",
            "games/solitaire/court-clubs-queen.webp",
            "games/solitaire/court-clubs-king.webp",
        ],
        _ => return None,
    };
    portraits.get(usize::from(rank.checked_sub(11)?)).copied()
}
