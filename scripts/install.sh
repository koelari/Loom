#!/bin/sh
set -eu

loom_project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
loom_bin_dir=${LOOM_BIN_DIR:-$HOME/.local/bin}
loom_data_dir=$HOME/.loom

case "$loom_bin_dir:$loom_data_dir" in
    /*:/*) ;;
    *) printf '%s\n' 'Install directories must be absolute.' >&2; exit 1 ;;
esac

cargo build --manifest-path "$loom_project_dir/Cargo.toml" --target-dir "$loom_project_dir/target" --release --locked
mkdir -p "$loom_bin_dir" "$loom_data_dir/tools" "$loom_data_dir/shell"
install -m 755 "$loom_project_dir/target/release/loom" "$loom_bin_dir/loom"
install -m 644 "$loom_project_dir/shell/init.sh" "$loom_data_dir/shell/init.sh"
printf 'Installed Loom: %s/loom\n' "$loom_bin_dir"
printf '%s\n' 'For current-shell activation, add this line to your shell startup configuration:'
printf '%s\n' 'source "$HOME/.loom/shell/init.sh"'
