pub const STAGE_COUNT: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub name: &'static str,
    pub hint: &'static str,
    pub shots: usize,
    pub par: usize,
    pub(crate) rack: &'static [&'static str],
}

pub const STAGES: [Stage; STAGE_COUNT] = [
    Stage {
        name: "First Light",
        hint: "Join three bubbles with the same mark to pop them.",
        shots: 8,
        par: 2,
        rack: &["..CCGG....", "..CCGG...", "....GG...."],
    },
    Stage {
        name: "Twin Shoals",
        hint: "Cut a cluster’s connection to the ceiling to drop the whole shoal.",
        shots: 10,
        par: 3,
        rack: &[
            ".GG....CC.",
            ".GG...CC.",
            "JJGG..CCVV",
            ".JJG..VV.",
            "....G.....",
        ],
    },
    Stage {
        name: "Sea Arch",
        hint: "Bank off either wall to curl a shot around the arch.",
        shots: 13,
        par: 5,
        rack: &[
            "CCGGGGGGCC",
            "CCVVVVVCC",
            "CCVV..VVCC",
            "JVV...VVJ",
            "JJ......JJ",
            "J.......J",
        ],
    },
    Stage {
        name: "Kelp Forest",
        hint: "Look for the narrow stems that hold up the branches.",
        shots: 15,
        par: 6,
        rack: &[
            "GG.CC.VJJJ",
            "GG.CC.VJJ",
            "VV.GG.VGJJ",
            "VJ.GG.CCV",
            ".JJ.JJ.CVV",
            ".JJ.J...V",
            "....J...V.",
        ],
    },
    Stage {
        name: "Undertow",
        hint: "Swap with the next bubble to reach an opening while it’s clear.",
        shots: 17,
        par: 7,
        rack: &[
            "CCVVJJGG..",
            ".CCVVJJG.",
            "..GGCVVGG.",
            "...GGCVVG",
            "...JJGCVVG",
            "....JJCCV",
            "....GGJCCC",
            "....GGJJ.",
        ],
    },
    Stage {
        name: "Deep Reef",
        hint: "Clear the low branches early to keep the warning line clear.",
        shots: 19,
        par: 8,
        rack: &[
            "VV.GG.CCJJ",
            "VV.GGCCCC",
            "VJVGVCGJJJ",
            "JJVVCGGJJ",
            "JCGVCVVGVV",
            "CCGGJJGGV",
            "..GJJ..GV.",
            "..VV...JJ",
            "...V...JJ.",
        ],
    },
];
