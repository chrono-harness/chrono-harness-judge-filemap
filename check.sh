#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [[ $# -eq 0 ]]; then
  /bin/sh .chrono-harness/ci/install.sh
  export PATH="$PWD/.chrono-harness/bin:$PATH"
  export CHRONO_CHECK_SOURCE=${CHRONO_CHECK_SOURCE:-local}
  exec .chrono-harness/bin/chrono-harness check
fi
if [[ $# -ne 1 || $1 != --native ]]; then
  echo 'usage: ./check.sh [--native]' >&2
  exit 2
fi
scratch_root=$(pwd -P)/.chrono-harness/state/native-check
mkdir -p "$scratch_root"
scratch=$(mktemp -d "$scratch_root/check.XXXXXXXX")
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

# Read the exact library selected by Cargo in the build of the tested executable.
metadata=$(jq -ers --arg manifest "$(pwd -P)/$product" '
  [.[] | select(.reason == "compiler-artifact" and .manifest_path == $manifest
                 and .target.kind == ["lib"]) | .filenames[] | select(endswith(".rmeta"))]
  | if length == 1 then .[0] else error("expected the product library") end
' "$scratch/build.jsonl") || coverage_failure "native library was not identified"
filename=${metadata##*/}
stem=${filename#lib}
stem=${metadata%/*}/${stem%.rmeta}
chrono-host-bootstrap check-compiler-sources native "$stem" src/lib.rs src/strict_json.rs src/verified_graph.rs src/request.rs src/registration.rs src/cost_inputs.rs src/graph_key.rs src/graph.rs src/workflow_paths.rs src/finding_origin.rs src/workflow.rs src/registration_environment.rs src/records.rs src/runtime.rs ||
  coverage_failure "native library differs from required sources"

# Both roots compile the same graph modules. The independent root also reads
# this deployed list when it checks the registered public graph source.
proof_modules=(main.rs records.rs strict_json.rs lib.rs workflow.rs runtime.rs cost_inputs.rs registration.rs request.rs registration_trace.rs registration_environment.rs finding_origin.rs workflow_paths.rs graph_key.rs classification.rs graph.rs)
production=crates/judge-filemap/src
independent=crates/judge-filemap-tests/tests
included=$independent/../../judge-filemap/src
product_inputs=("$production/verified_graph.rs")
roadmap_inputs="$independent/../../../.chrono-harness/ci"
test_inputs=("$independent/verified_graph.rs" "$roadmap_inputs/roadmap.rs" "$included/verified_graph.rs")
for module in "${proof_modules[@]}"; do
  product_inputs+=("$production/$module")
  test_inputs+=("$included/$module")
done
test_inputs+=("$independent/support/registration.rs" "$roadmap_inputs/../../.github/roadmap.cjs" "$roadmap_inputs/../../.github/workflows/issue-roadmap.yml"
  "$roadmap_inputs/../../.github/workflows/actions.lock" "$independent/../../../check.sh")
chrono-host-bootstrap check-compiler-sources proof .chrono-harness/state/filemap-proofs/verify.filemap-graph.rlib \
  "${product_inputs[@]}" || coverage_failure "production graph proof has different compiler inputs"
chrono-host-bootstrap check-compiler-sources proof .chrono-harness/state/filemap-proofs/verify.filemap-graph-tests.rlib \
  "${test_inputs[@]}" || coverage_failure "expectation proof has different compiler inputs"
