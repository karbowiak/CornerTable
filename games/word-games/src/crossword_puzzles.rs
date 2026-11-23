use crate::crossword::{Clue, CrosswordPuzzle};

const fn c(answer: &'static str, text: &'static str, _alternates: &'static [&'static str]) -> Clue {
    Clue {
        answer,
        text,
        #[cfg(test)]
        alternates: _alternates,
    }
}

pub(crate) const MINIS: &[CrosswordPuzzle] = &[
    CrosswordPuzzle {
        across: &[
            c("CAT", "Purring pet", &[]),
            c("ORE", "Rock mined for its metal", &[]),
            c("DEN", "A fox's home", &[]),
        ],
        down: &[
            c("COD", "White fish often battered for fish and chips", &[]),
            c("ARE", "They ___ ready to leave", &[]),
            c("TEN", "Number of toes on two bare feet", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("TAP", "Faucet", &[]),
            c("ORE", "Source of iron in a mine", &[]),
            c("WET", "Still damp from the rain", &[]),
        ],
        down: &[
            c("TOW", "Pull a trailer behind a car", &[]),
            c("ARE", "You ___ here, on a map", &[]),
            c("PET", "Animal that lives with you", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("BAY", "Inlet larger than a cove", &[]),
            c("ICE", "Frozen water popped from a tray", &[]),
            c("TEA", "Drink steeped in a pot", &[]),
        ],
        down: &[
            c("BIT", "A tiny piece", &[]),
            c("ACE", "Rank above king in many card games", &[]),
            c("YEA", "An affirmative vote called aloud", &["AYE"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ERR", "Make a mistake", &[]),
            c("VIA", "By way of", &[]),
            c("EGG", "A hen's contribution to breakfast", &[]),
        ],
        down: &[
            c("EVE", "Day before a holiday", &[]),
            c("RIG", "Structure used to drill for oil", &[]),
            c("RAG", "Old cloth for wiping up spills", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("RAN", "Moved at a sprint, in the past", &[]),
            c("OWE", "Have yet to pay back", &[]),
            c("DEW", "Drops on the morning grass", &[]),
        ],
        down: &[
            c("ROD", "Pole used for fishing", &[]),
            c("AWE", "Wonder at the northern lights", &[]),
            c("NEW", "Fresh off the shelf", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("EGO", "What a big head might have too much of", &[]),
            c("RIP", "Tear paper in two", &[]),
            c("ANT", "Picnic visitor that lives in a colony", &[]),
        ],
        down: &[
            c("ERA", "A period in history", &["AGE", "EON"]),
            c("GIN", "Spirit flavored with juniper", &[]),
            c("OPT", "Choose, as in ___ in", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("SPA", "Place to book a massage", &[]),
            c("ART", "What a gallery displays", &[]),
            c("DYE", "Coloring for cloth", &[]),
        ],
        down: &[
            c("SAD", "Feeling blue", &[]),
            c("PRY", "Open with a crowbar", &[]),
            c("ATE", "Had lunch, say", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("CAB", "Taxi", &[]),
            c("USE", "Put to work", &[]),
            c("THE", "Definite article in English", &[]),
        ],
        down: &[
            c("CUT", "What scissors do", &[]),
            c("ASH", "Powder left by a fire", &[]),
            c("BEE", "Insect in a hive", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("OFF", "Switch position opposite on", &[]),
            c("WEE", "Very small, as in the ___ hours", &[]),
            c("LED", "Was out in front, for a time", &[]),
        ],
        down: &[
            c("OWL", "Night bird with a hoot", &[]),
            c("FEE", "Charge for a service", &[]),
            c("FED", "Gave the fish their flakes", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("SIP", "Drink in tiny amounts", &[]),
            c("PRO", "Paid athlete, informally", &[]),
            c("YET", "So far, as in ‘It hasn't arrived ___’", &[]),
        ],
        down: &[
            c("SPY", "Agent who gathers secrets", &[]),
            c("IRE", "Anger in a headline", &[]),
            c("POT", "Stovetop vessel for a stew", &["PAN"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("SOW", "Scatter seeds for planting", &[]),
            c("AIR", "What lungs take in", &[]),
            c("PLY", "One layer of laminated wood", &[]),
        ],
        down: &[
            c("SAP", "What flows out of a tapped maple", &[]),
            c("OIL", "Liquid that keeps an engine lubricated", &[]),
            c("WRY", "Dryly humorous", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("RYE", "Grain in pumpernickel bread", &[]),
            c("ION", "Electrically charged atom", &[]),
            c("MUD", "What rain makes of dirt", &[]),
        ],
        down: &[
            c("RIM", "Top edge of a drinking glass", &["LIP"]),
            c("YOU", "Second-person pronoun", &[]),
            c("END", "The finish, opposite the start", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("SHE", "What ‘Maria’ becomes in ‘Maria is here’", &[]),
            c("EEL", "Long, snake-like fish", &[]),
            c("ARM", "Limb from shoulder to fingertips", &[]),
        ],
        down: &[
            c("SEA", "The Mediterranean, for one", &[]),
            c("HER", "I spoke with Maria. I spoke with ___", &[]),
            c("ELM", "Shade tree whose name rhymes with helm", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ORB", "A sphere, such as a crystal ball", &[]),
            c("DAY", "A 24-hour stretch", &[]),
            c("EYE", "Organ that detects light", &[]),
        ],
        down: &[
            c("ODE", "Poem written in praise of something", &[]),
            c("RAY", "Beam of sunshine", &[]),
            c("BYE", "What to say when hanging up", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ITS", "The kitten chased ___ tail", &[]),
            c("LIE", "An intentional falsehood", &[]),
            c("LET", "Allow someone to do something", &[]),
        ],
        down: &[
            c("ILL", "Feeling under the weather", &[]),
            c("TIE", "Neckwear knotted under a collar", &[]),
            c("SET", "Go down beyond the horizon, as the sun does", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("BIG", "What ‘large’ means in fewer letters", &[]),
            c("EVE", "Night before the new year", &[]),
            c("GYM", "Room full of treadmills and weights", &[]),
        ],
        down: &[
            c("BEG", "Plead for a favor", &["ASK"]),
            c("IVY", "Plant that climbs brick walls", &[]),
            c("GEM", "A cut ruby or emerald", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ROE", "Fish eggs on sushi", &[]),
            c("INN", "Small lodging for travelers", &[]),
            c("BED", "Furniture with a mattress", &["COT"]),
        ],
        down: &[
            c("RIB", "Bone that helps enclose the lungs", &[]),
            c("ONE", "Half of two", &[]),
            c("END", "Part of a pencil with the eraser", &["TOP"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ARC", "Curving part of a circle", &[]),
            c("TOO", "Excessively, as in ‘___ loud’", &[]),
            c("EBB", "Recede, as the tide", &[]),
        ],
        down: &[
            c("ATE", "Chewed and swallowed", &[]),
            c("ROB", "Steal from someone", &[]),
            c("COB", "What the kernels grow on", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("POD", "Case around a row of peas", &[]),
            c("EAR", "Body part with a lobe", &[]),
            c("PRY", "Nose into someone else's business", &[]),
        ],
        down: &[
            c("PEP", "Energy shown in a brisk walk", &[]),
            c("OAR", "Blade used in rowing", &[]),
            c("DRY", "No longer wet", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("THY", "Old-fashioned ‘your’ before ‘name’", &[]),
            c("WOE", "Deep sorrow", &[]),
            c("OWN", "Have as one's property", &[]),
        ],
        down: &[
            c("TWO", "The only even prime number", &[]),
            c("HOW", "In what way?", &[]),
            c("YEN", "Japan's currency", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("MEN", "Gentlemen", &[]),
            c("AGE", "Number marked by a birthday", &[]),
            c("DOT", "Small mark above a lowercase i", &[]),
        ],
        down: &[
            c("MAD", "Angry enough to say ‘I'm ___ at you’", &[]),
            c("EGO", "Part of the mind between id and superego", &[]),
            c("NET", "What hangs from a basketball hoop", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("PIT", "Hard center of a peach", &[]),
            c("ICE", "Cubes that chill a drink", &[]),
            c("EYE", "Hole the thread goes through", &[]),
        ],
        down: &[
            c("PIE", "Dessert with a crust and filling", &[]),
            c("ICY", "Slick with frozen water", &[]),
            c("TEE", "Peg a golfer sets the ball on", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("MAY", "Month after April", &[]),
            c("OWE", "Be in debt to", &[]),
            c("PEW", "Bench in a church", &[]),
        ],
        down: &[
            c("MOP", "Tool for washing a floor", &[]),
            c("AWE", "Feeling when you first see the Grand Canyon", &[]),
            c(
                "YEW",
                "Evergreen traditionally used to make longbows",
                &["ASH", "ELM"],
            ),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("JIG", "Lively dance with quick steps", &[]),
            c("ORE", "Metal-bearing rock", &[]),
            c("BET", "Wager on a game", &[]),
        ],
        down: &[
            c("JOB", "Paid position at a company", &[]),
            c("IRE", "Three-letter word meaning wrath", &[]),
            c("GET", "Receive, as a gift", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("ASH", "Pale wood used for many baseball bats", &["OAK"]),
            c("PEA", "A single round, green vegetable from a pod", &[]),
            c("EAT", "Dine", &[]),
        ],
        down: &[
            c("APE", "Primate such as a gibbon or gorilla", &[]),
            c("SEA", "Large body of salt water", &[]),
            c("HAT", "What a fedora or bowler might be", &["CAP"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("FUR", "Coat that keeps a fox warm", &[]),
            c("ORE", "What a gold mine yields before processing", &[]),
            c("END", "Opposite of beginning", &[]),
        ],
        down: &[
            c("FOE", "An enemy, opposite a friend", &[]),
            c("URN", "Tall vessel for serving coffee to a crowd", &[]),
            c("RED", "Color of a ripe tomato", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("OLD", "Far from new", &[]),
            c("DUO", "Pair on a concert bill", &[]),
            c("EGG", "What the shell and yolk are parts of", &[]),
        ],
        down: &[
            c("ODE", "Poem such as ‘To a Nightingale’", &[]),
            c("LUG", "Carry something heavy with effort", &[]),
            c("DOG", "Pet that wags its tail", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("OWL", "Hooting hunter with a swivelling head", &[]),
            c("ROE", "Fish eggs served on sushi", &[]),
            c("BED", "Place to sleep under a quilt", &["COT"]),
        ],
        down: &[
            c("ORB", "A globe or sphere", &[]),
            c("WOE", "Sorrow in an old ballad", &[]),
            c("LED", "Energy-saving light in a lantern, for short", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("BAT", "Flying mammal that uses echoes to navigate", &[]),
            c("ERA", "A period in history", &["AGE"]),
            c("DEN", "A fox’s underground home", &[]),
        ],
        down: &[
            c("BED", "Garden plot for flowers", &[]),
            c("ARE", "You ___ here, on a map", &[]),
            c("TAN", "Light brown, like sand after it dries", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("FOG", "A cloud right down at street level", &[]),
            c("OWE", "Have a debt to repay", &[]),
            c("ELM", "Tree in the name of a certain nightmare street", &[]),
        ],
        down: &[
            c("FOE", "An enemy or adversary", &[]),
            c("OWL", "A barn ___ hunts by night", &[]),
            c("GEM", "Stone such as an opal or ruby", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("SKY", "Where you see the moon", &[]),
            c("PIE", "Fruit dessert with a pastry crust", &[]),
            c("ANT", "Picnic visitor with six legs", &["FLY"]),
        ],
        down: &[
            c("SPA", "Place for massages and steam baths", &[]),
            c("KIN", "Relatives, collectively", &[]),
            c("YET", "Have you seen the stars ___?", &[]),
        ],
    },
];

pub(crate) const LARGER: &[CrosswordPuzzle] = &[
    CrosswordPuzzle {
        across: &[
            c("CHOP", "Cut into pieces with a knife", &["DICE"]),
            c("RAVE", "Speak enthusiastically about a new favorite", &[]),
            c("ORAL", "Spoken, as an exam might be", &[]),
            c("WELT", "Raised mark left by a blow to the skin", &["BUMP"]),
        ],
        down: &[
            c("CROW", "Black bird that caws", &["ROOK"]),
            c("HARE", "The tortoise's racing rival", &[]),
            c("OVAL", "Egg-shaped outline", &[]),
            c("PELT", "Animal skin with the fur still on", &["HIDE"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("BUSH", "Leafy shrub", &[]),
            c("ONTO", "The cat leapt ___ the table", &["OVER"]),
            c(
                "ODOR",
                "Smell, especially one you'd rather avoid",
                &["REEK"],
            ),
            c("TOWN", "Settlement bigger than a village", &["CITY"]),
        ],
        down: &[
            c("BOOT", "Footwear for a muddy hike", &["SHOE"]),
            c("UNDO", "Reverse the last change in an editor", &[]),
            c("STOW", "Put luggage away in a compartment", &["PACK"]),
            c("HORN", "What a rhino has on its snout", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("TWIG", "Small branch at the end of a stem", &[]),
            c("RISE", "What dough does in a warm spot", &["PUFF"]),
            c("IDLE", "Run while the car is parked, as an engine can", &[]),
            c("MEEK", "Gentle and unlikely to push back", &["MILD"]),
        ],
        down: &[
            c("TRIM", "Tidy the edges of a hedge", &["CLIP", "SNIP"]),
            c("WIDE", "Measuring a lot from side to side", &[]),
            c("ISLE", "Small island, as in ‘___ of Skye’", &[]),
            c("GEEK", "Enthusiast, as in ‘a train ___’", &["NERD"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("STOP", "Command on a red octagonal sign", &[]),
            c("LIVE", "Broadcast as it happens", &[]),
            c("IDEA", "Thought that might spark a project", &[]),
            c("PERK", "Job benefit, like free lunches", &[]),
        ],
        down: &[
            c("SLIP", "Slide and lose your footing", &["SKID"]),
            c("TIDE", "Daily rise and fall of the sea", &[]),
            c("OVER", "Finished, as a meeting could be", &["DONE"]),
            c("PEAK", "Highest point of a mountain", &["APEX"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("GOSH", "Mild exclamation often paired with ‘golly’", &[]),
            c("UNTO", "Do ___ others, in the Golden Rule", &[]),
            c("SCAM", "Swindle or deceptive scheme", &["RUSE"]),
            c("HERE", "In this place", &[]),
        ],
        down: &[
            c("GUSH", "Pour out in a rush", &["FLOW"]),
            c("ONCE", "One time only", &[]),
            c("STAR", "What the sun is", &[]),
            c("HOME", "Where the heart is, in a saying", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("STEW", "Slow-cooked dish in a thick broth", &[]),
            c("PAVE", "Lay asphalt on a road", &[]),
            c("OXEN", "Pair of draft cattle in a yoke", &[]),
            c("TINT", "A slight color, as on a window", &[]),
        ],
        down: &[
            c("SPOT", "A single mark on a ladybug", &[]),
            c("TAXI", "Hire car with a meter", &[]),
            c("EVEN", "Divisible by two", &[]),
            c("WENT", "Past tense of ‘go’", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("GRID", "Layout of a crossword's squares", &[]),
            c("RODE", "Traveled on a horse", &[]),
            c("UPON", "Once ___ a time", &[]),
            c("BELT", "Waist strap held by a buckle", &[]),
        ],
        down: &[
            c("GRUB", "Insect larva you might dig up", &[]),
            c("ROPE", "Thick cord used on a sailboat", &["LINE"]),
            c("IDOL", "Someone an admirer looks up to", &["HERO"]),
            c(
                "DENT",
                "Inward mark from a bump, as in a car door",
                &["DING"],
            ),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("STEM", "Part of a plant that supports the leaves", &[]),
            c("PUMA", "Large cat also called a cougar", &[]),
            c("UNIT", "A measure such as a meter", &[]),
            c("DATE", "Sweet fruit from a palm tree", &[]),
        ],
        down: &[
            c("SPUD", "Informal name for a potato", &[]),
            c("TUNA", "Fish often used in a melt", &[]),
            c("EMIT", "Give off, as a light or sound", &[]),
            c("MATE", "One of a matching pair", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("PROP", "Handheld object in a stage production", &[]),
            c("RAKE", "Garden tool that gathers fallen leaves", &[]),
            c("OGRE", "Man-eating giant of fairy tales", &[]),
            c("MEAL", "Breakfast, lunch, or dinner", &[]),
        ],
        down: &[
            c("PROM", "Formal dance held at a high school", &[]),
            c("RAGE", "Fury", &[]),
            c("OKRA", "Long green pod used to thicken gumbo", &[]),
            c("PEEL", "Outer skin of an orange", &["RIND"]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("GRIM", "Bleak, as bad news may be", &["DOUR"]),
            c("LUTE", "Plucked instrument with a pear-shaped back", &[]),
            c("USER", "Person with an account on a site", &[]),
            c("MEME", "Joke image spread and adapted online", &[]),
        ],
        down: &[
            c("GLUM", "Down in the dumps", &["BLUE"]),
            c("RUSE", "Trick meant to fool someone", &["SCAM"]),
            c("ITEM", "One entry on a shopping list", &[]),
            c("MERE", "Nothing more than, as in ‘a ___ coincidence’", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("MOON", "Earth’s natural satellite", &[]),
            c("UNDO", "Reverse an edit", &[]),
            c("STOP", "The red octagonal sign’s instruction", &[]),
            c("TORE", "Ripped through paper", &[]),
        ],
        down: &[
            c("MUST", "Freshly pressed juice before it becomes wine", &[]),
            c("ONTO", "The cat jumped ___ the roof", &["OVER"]),
            c("ODOR", "A smell picked up by the nose", &["REEK"]),
            c("NOPE", "Casual way to say absolutely not", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("TRAM", "A streetcar on rails", &[]),
            c("HIRE", "Take someone on for a job", &[]),
            c("IDEA", "A light bulb moment", &[]),
            c("NEAT", "Tidy and well ordered", &[]),
        ],
        down: &[
            c("THIN", "Like paper compared with cardboard", &[]),
            c("RIDE", "Go along as a passenger", &[]),
            c("AREA", "Space measured in square metres", &[]),
            c("MEAT", "What a butcher cuts and sells", &[]),
        ],
    },
    CrosswordPuzzle {
        across: &[
            c("MOTH", "Insect drawn to a porch light", &[]),
            c("AREA", "Named part of a city", &["ZONE"]),
            c("SCAR", "Mark left by a healed cut", &[]),
            c("TALE", "Story told by the fire", &["YARN"]),
        ],
        down: &[
            c("MAST", "Upright pole that holds a sail", &[]),
            c("ORCA", "Black-and-white whale", &[]),
            c("TEAL", "Blue-green color, or a small duck", &[]),
            c("HARE", "Long-eared runner in a fable", &[]),
        ],
    },
];
