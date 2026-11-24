#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p target
scratch=$(mktemp -d "$(pwd -P)/target/check.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
export TMPDIR="$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$scratch/cargo}"
cargo fmt --check --manifest-path crates/judge-filemap/Cargo.toml
product=crates/judge-filemap/Cargo.toml
cargo build --locked --message-format=json-render-diagnostics --manifest-path "$product" > "$scratch/build.jsonl"
cargo fmt --check --manifest-path crates/judge-filemap-tests/Cargo.toml
CHRONO_TEST_GIT=$(command -v chrono-test-git) || { echo "chrono-test-git is required by filemap tests" >&2; exit 1; }
export CHRONO_TEST_GIT
# Cargo invokes the test executables with the fixture first on PATH.
# Build scripts and the compiler keep their normal environment.
mkdir "$scratch/test-bin"
ln -s "$CHRONO_TEST_GIT" "$scratch/test-bin/git"
test_runner=$(jq -cn --arg path "PATH=$scratch/test-bin:$PATH" '["/usr/bin/env", $path]')
cargo test --locked --manifest-path crates/judge-filemap-tests/Cargo.toml --config "target.'cfg(all())'.runner=$test_runner"
coverage_failure() {
  echo "E_VERUS_COVERAGE: $*" >&2
  exit 1
}

verifier=$(command -v "${VERUS:-verus}") || coverage_failure "registered verifier was not found"

prove() {
  chrono-host-bootstrap run-compiler-proof "$scratch" "$1" "$verifier" "$2" ||
    coverage_failure "$1: verifier failed"
}

admit_proof() {
  local operation=$1
  shift
  chrono-host-bootstrap collect-compiler-proof "$scratch" "$operation" "$verifier" \
    0.2026.04.24.f8e1704 f8e17045c4ac17a02e566b8997070f2eecc255be "$@" ||
    coverage_failure "$operation: required proof result was not admitted"
}

# Read the exact library selected by Cargo in the build of the tested executable.
metadata=$(jq -ers --arg manifest "$(pwd -P)/$product" '
  [.[] | select(.reason == "compiler-artifact" and .manifest_path == $manifest
                 and .target.kind == ["lib"]) | .filenames[] | select(endswith(".rmeta"))]
  | if length == 1 then .[0] else error("expected the product library") end
' "$scratch/build.jsonl") || coverage_failure "native library was not identified"
filename=${metadata##*/}
stem=${filename#lib}
stem=${metadata%/*}/${stem%.rmeta}
chrono-host-bootstrap check-compiler-sources native "$stem" src/verified_graph.rs src/graph_key.rs src/graph.rs ||
  coverage_failure "native library differs from required sources"

graph=crates/judge-filemap/src/verified_graph.rs
keys=crates/judge-filemap/src/graph_key.rs
expectations=crates/judge-filemap-tests/tests/verified_graph.rs
include=crates/judge-filemap-tests/tests/../../judge-filemap/src

product_operation=verify.filemap-graph
test_operation=verify.filemap-graph-tests

prove "$product_operation" "$graph"
prove "$test_operation" "$expectations"

admit_proof "$product_operation" "$graph" "$keys"
admit_proof "$test_operation" "$expectations" "$include/verified_graph.rs" "$include/graph_key.rs"
