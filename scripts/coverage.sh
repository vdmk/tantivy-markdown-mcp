#!/bin/sh

set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
coverage_dir="$repository_root/target/coverage"
binary="$coverage_dir/debug/tantivy-markdown-mcp"
rust_llvm="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin"

mkdir -p "$coverage_dir"
rm -f "$coverage_dir"/*.profraw "$coverage_dir/merged.profdata"

LLVM_PROFILE_FILE="$coverage_dir/build-%p-%m.profraw" \
RUSTFLAGS="-C instrument-coverage" \
cargo build --target-dir "$coverage_dir" --bin tantivy-markdown-mcp >/dev/null 2>&1

LLVM_PROFILE_FILE="$coverage_dir/%p-%m.profraw" \
MCP_TEST_BINARY="$binary" \
cargo test --all-targets

"$rust_llvm/llvm-profdata" merge -sparse "$coverage_dir"/*.profraw \
    -o "$coverage_dir/merged.profdata"

"$rust_llvm/llvm-cov" report "$binary" \
    -instr-profile="$coverage_dir/merged.profdata" \
    -ignore-filename-regex='(/.cargo/registry/|/rustc/)'
