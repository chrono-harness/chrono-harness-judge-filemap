#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$scratch/cargo}"
cargo fmt --check --manifest-path crates/judge-filemap/Cargo.toml
cargo build --locked --manifest-path crates/judge-filemap/Cargo.toml
cargo fmt --check --manifest-path crates/judge-filemap-tests/Cargo.toml
CHRONO_TEST_GIT=$(command -v chrono-test-git) || { echo "chrono-test-git is required by filemap tests" >&2; exit 1; }
export CHRONO_TEST_GIT
# Cargo invokes the test executables with the fixture first on PATH.
# Build scripts and the compiler keep their normal environment.
mkdir "$scratch/test-bin"
ln -s "$CHRONO_TEST_GIT" "$scratch/test-bin/git"
test_runner=$(jq -cn --arg path "PATH=$scratch/test-bin:$PATH" '["/usr/bin/env", $path]')
cargo test --locked --manifest-path crates/judge-filemap-tests/Cargo.toml --config "target.'cfg(all())'.runner=$test_runner"
prove() {
  local label=$1 root=$2
  local output="$scratch/$label.rlib"
  "${VERUS:-verus}" --crate-type=lib --no-cheating --no-external-by-default --num-threads 2 \
    --output-json --compile "--emit=link=$output,dep-info=$output.d" "$root" -o "$output" \
    -Zchecksum-hash-algorithm=sha256 > "$scratch/proof.json" || { cat "$scratch/proof.json" >&2; return 1; }
  cat "$scratch/proof.json"
  jq -e '.verus.version == "0.2026.04.24.f8e1704" and
    .verus.commit == "f8e17045c4ac17a02e566b8997070f2eecc255be" and
    (."verification-results" | .success == true and ."is-verifying-entire-crate" == true and
    ."encountered-error" == false and ."encountered-vir-error" == false and
    .errors == 0 and .verified > 0)' "$scratch/proof.json" >/dev/null
  test -s "$output" && test -s "$output.d"
}

prove verify.filemap-graph crates/judge-filemap/src/verified_graph.rs
prove verify.filemap-graph-tests crates/judge-filemap-tests/tests/verified_graph.rs
