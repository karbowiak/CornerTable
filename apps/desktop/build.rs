use std::{env, fs, path::PathBuf};

const LOCAL_AUDIO: &[&str] = &[
    "move.wav",
    "invalid-move.wav",
    "flag.wav",
    "unflag.wav",
    "bubble-pop.wav",
    "bubble-chain.wav",
    "gem-clear.wav",
    "gem-chain.wav",
    "gem-chain-long.wav",
];

fn main() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=local-audio");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    for name in LOCAL_AUDIO {
        let local = PathBuf::from("local-audio").join(name);
        let input = if local.is_file() {
            local
        } else {
            PathBuf::from("assets/sounds").join(name)
        };
        fs::copy(&input, output.join(name))
            .unwrap_or_else(|error| panic!("Could not embed {}: {error}", input.display()));
    }
}
