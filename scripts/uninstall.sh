#!/bin/sh
set -eu

loom_bin_dir=${LOOM_BIN_DIR:-$HOME/.local/bin}
loom_data_dir=$HOME/.loom

case "$loom_bin_dir:$loom_data_dir" in
    /*:/*) ;;
    *) printf '%s\n' 'Install directories must be absolute.' >&2; exit 1 ;;
esac

rm -f "$loom_bin_dir/loom" "$loom_data_dir/shell/init.sh"
# Remove only empty integration directories; keep tools and other user files.
rmdir "$loom_data_dir/shell" 2>/dev/null || :
rmdir "$loom_data_dir" 2>/dev/null || :

printf '%s\n' 'Loom uninstalled.'
printf 'Tools and declarations under %s/tools are preserved.\n' "$loom_data_dir"
printf '%s\n' 'Remove this line from your shell startup configuration, then restart the shell:'
printf '%s\n' 'source "$HOME/.loom/shell/init.sh"'
