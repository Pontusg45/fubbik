#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# The database-backed workspace creates many test binaries. Keep the check
# reproducible on CI runners and small local disks without retaining 10+ GiB
# of incremental objects and debug symbols.
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
export CARGO_PROFILE_TEST_DEBUG="${CARGO_PROFILE_TEST_DEBUG:-0}"

managed_db=false
test_log="$(mktemp)"
cleanup() {
    if "$managed_db"; then
        ./scripts/test/rust-test-db.sh stop || true
    fi
    rm -f "$test_log"
}
trap cleanup EXIT

if [[ "${FUBBIK_TEST_DB_EXTERNAL:-false}" == "true" ]]; then
    if [[ -z "${DATABASE_URL:-}" ]]; then
        echo 'DATABASE_URL is required for external database mode. Run: ./scripts/test/rust-test-db.sh start && DATABASE_URL="$(./scripts/test/rust-test-db.sh url)" FUBBIK_TEST_DB_EXTERNAL=true just rust-test' >&2
        exit 1
    fi
    psql "$DATABASE_URL" -Atqc 'SELECT 1' | grep -qx '1' || {
        echo "External test database is not reachable at DATABASE_URL. Run: ./scripts/test/rust-test-db.sh start" >&2
        exit 1
    }
else
    ./scripts/test/rust-test-db.sh start >/dev/null
    managed_db=true
    export DATABASE_URL="$(./scripts/test/rust-test-db.sh url)"
fi
export SQLX_OFFLINE=true

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
node --test scripts/check-rust-migration.test.mjs
node scripts/check-rust-migration.mjs

result=0
cargo test --workspace --no-fail-fast 2>&1 | tee "$test_log" || result=$?
awk '
    /^test result:/ {
        passed += $4
        failed += $6
        ignored += $8
    }
    END {
        printf "Rust test totals: %d passed, %d failed, %d ignored\n", passed, failed, ignored
    }
' "$test_log"
exit "$result"
