#!/bin/sh

set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

echo "Scanning Git history for secrets..."
gitleaks git --redact

echo "Checking Rust dependencies for known vulnerabilities..."
cargo audit
