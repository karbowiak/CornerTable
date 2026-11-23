#!/usr/bin/env bash
set -euo pipefail

mode=${1:-all}
case "$mode" in
    style|all)
        cargo fmt --all -- --check
        cargo clippy --locked --workspace --all-targets -- -D warnings
        ;;
    *)
        echo 'Usage: scripts/check.sh [style|all]' >&2
        exit 2
        ;;
esac

if [[ "$mode" == all ]]; then
    unset CORNER_TABLE_DATA_DIR
    cargo test --locked --workspace
fi
