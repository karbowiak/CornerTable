#[path = "src/search_catalog.rs"]
mod search_catalog;
#[path = "src/search_generate.rs"]
mod search_generate;

use std::{env, fs, path::PathBuf};

use search_catalog::{LAYOUTS_PER_THEME, THEMES};

fn main() {
    println!("cargo:rerun-if-changed=src/search_catalog.rs");
    println!("cargo:rerun-if-changed=src/search_generate.rs");
    println!("cargo:rerun-if-changed=build.rs");

    // Layouts are generated and certified at build time. The seed and
    // placement algorithm form part of the saved puzzle format.
    let mut output = String::from("&[\n");
    for (index, theme) in THEMES.iter().enumerate() {
        assert!(
            theme.words.len() >= search_generate::DIRECTIONS.len()
                && theme.words.windows(2).all(|pair| pair[0] < pair[1])
                && theme.words.iter().all(|word| {
                    (3..=theme.side).contains(&word.len())
                        && word.bytes().all(|letter| letter.is_ascii_uppercase())
                }),
            "invalid word list for {}",
            theme.title
        );
        output.push_str("  [\n");
        for layout in 0..LAYOUTS_PER_THEME {
            let seed = 0x85f3_9a23_44bd_6211 ^ ((index as u64) << 32) ^ layout as u64;
            let search_generate::Layout { grid, paths } = search_generate::generate(*theme, seed)
                .unwrap_or_else(|| {
                    panic!(
                        "failed to create a unique {} layout for seed {seed}",
                        theme.title
                    )
                });
            let grid = String::from_utf8(grid).unwrap();
            output.push_str(&format!("    (b{grid:?}, &{paths:?}),\n"));
        }
        output.push_str("  ],\n");
    }
    output.push(']');
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("word_search_boards.rs"), output).unwrap();
}
