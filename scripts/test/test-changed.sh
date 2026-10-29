#!/usr/bin/env bash
# Test only the Rust crates changed since a base ref, plus their transitive
# reverse dependencies inside the workspace.
#
# Usage: test-changed.sh [--dry-run] [base]     (base defaults to main)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

dry_run=false
base=main
for arg in "$@"; do
    case "$arg" in
        --dry-run) dry_run=true ;;
        -*) echo "unknown option: $arg" >&2; exit 2 ;;
        *) base="$arg" ;;
    esac
done

merge_base="$(git merge-base "$base" HEAD)"

# Committed + uncommitted tracked changes since the merge base, plus untracked files.
changed_files="$( { git diff --name-only "$merge_base"; git ls-files --others --exclude-standard; } | sort -u)"

if [[ -z "$changed_files" ]]; then
    echo "nothing changed since $base"
    exit 0
fi

# Changes outside crates/ that affect every Rust crate.
rust_global_re='^(Cargo\.toml|Cargo\.lock|rust-toolchain(\.toml)?|clippy\.toml|rustfmt\.toml|\.rustfmt\.toml|\.cargo/)'
test_all=false
if grep -Eq "$rust_global_re" <<<"$changed_files"; then
    test_all=true
fi

web_changed=false
if grep -Eq '^(apps/web/|packages/)' <<<"$changed_files"; then
    web_changed=true
fi

# Map changed files to workspace packages and close over reverse dependencies.
# Prints one package name per line.
select_crates() {
    local files_json
    files_json="$(printf '%s\n' "$changed_files" | jq -R . | jq -s .)"
    cargo metadata --format-version 1 --no-deps | jq -r \
        --argjson files "$files_json" --arg root "$repo_root" '
        [.packages[] | {
            name,
            dir: (.manifest_path | ltrimstr($root + "/") | sub("/?Cargo\\.toml$"; "")),
            deps: [.dependencies[].name]
        }] as $pkgs
        | [$pkgs[] | select(.dir != "" and (.dir as $d | any($files[]; startswith($d + "/")))) | .name] as $seed
        | def close($set):
            ([$pkgs[] | select(any(.deps[]; . as $d | $set | index($d))) | .name] + $set | unique) as $next
            | if ($next | length) == ($set | length) then $set else close($next) end;
          close($seed)[]'
}

all_workspace_crates() {
    cargo metadata --format-version 1 --no-deps | jq -r '.packages[].name'
}

echo "base: $base (merge-base ${merge_base:0:12})"
echo "changed files: $(wc -l <<<"$changed_files" | tr -d ' ')"

crates=()
if $test_all; then
    echo "Rust-wide file changed ($(grep -E "$rust_global_re" <<<"$changed_files" | paste -sd' ' -)): testing ALL crates via scripts/test/test-rust.sh"
else
    while IFS= read -r c; do
        [[ -n "$c" ]] && crates+=("$c")
    done < <(select_crates)
    if ((${#crates[@]})); then
        echo "crates to test (changed + reverse dependencies): ${crates[*]}"
    else
        echo "no Rust crates affected"
    fi
fi
if $web_changed; then
    echo "web/packages changed: will run pnpm run check-types and web unit tests"
fi

if ! $test_all && ((${#crates[@]} == 0)) && ! $web_changed; then
    echo "no testable changes since $base"
    exit 0
fi

if $dry_run; then
    echo "(dry run: nothing executed)"
    exit 0
fi

if $test_all; then
    ./scripts/test/test-rust.sh
elif ((${#crates[@]})); then
    export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
    export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
    export CARGO_PROFILE_TEST_DEBUG="${CARGO_PROFILE_TEST_DEBUG:-0}"
    export SQLX_OFFLINE=true

    needs_db=false
    for c in "${crates[@]}"; do
        case "$c" in fubbik-db | fubbik-api | fubbik) needs_db=true ;; esac
    done

    managed_db=false
    cleanup() {
        if "$managed_db"; then
            ./scripts/test/rust-test-db.sh stop || true
        fi
    }
    trap cleanup EXIT

    if $needs_db; then
        if [[ "${FUBBIK_TEST_DB_EXTERNAL:-false}" == "true" ]]; then
            if [[ -z "${DATABASE_URL:-}" ]]; then
                echo 'DATABASE_URL is required for external database mode.' >&2
                exit 1
            fi
        else
            ./scripts/test/rust-test-db.sh start >/dev/null
            managed_db=true
            export DATABASE_URL="$(./scripts/test/rust-test-db.sh url)"
        fi
    fi

    pkg_args=()
    for c in "${crates[@]}"; do pkg_args+=(-p "$c"); done
    cargo test "${pkg_args[@]}" --no-fail-fast
fi

if $web_changed; then
    pnpm run check-types
    pnpm --dir apps/web run test
fi
