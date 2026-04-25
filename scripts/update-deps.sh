#!/usr/bin/env bash
#
# update-deps.sh — Bump all dependencies to their latest versions,
# verify the build, and optionally increment the app's patch version
# and push to GitHub with a version tag.
#
# The version is only bumped if dependencies actually changed.
#
# Usage:
#   ./update-deps.sh
#
# Prerequisites:
#   - cargo-edit (provides `cargo upgrade` and `cargo set-version`)
#     Install with: cargo install cargo-edit

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

info()  { printf "%b[INFO]%b  %s\n" "$CYAN" "$RESET" "$*"; }
ok()    { printf "%b[OK]%b    %s\n" "$GREEN" "$RESET" "$*"; }
warn()  { printf "%b[WARN]%b  %s\n" "$YELLOW" "$RESET" "$*"; }
error() { printf "%b[ERROR]%b %s\n" "$RED" "$RESET" "$*"; }
step()  { printf "\n%b── Step %s ──%b\n" "$BOLD" "$1" "$RESET"; }

# ---------------------------------------------------------------------------
# Ensure we're in the project root (where Cargo.toml lives)
# ---------------------------------------------------------------------------
if [[ ! -f Cargo.toml ]]; then
    error "Cargo.toml not found. Run this script from the project root."
    exit 1
fi

# ---------------------------------------------------------------------------
# Prerequisites: check for cargo-edit
# ---------------------------------------------------------------------------
if ! cargo upgrade --version &>/dev/null; then
    error "cargo-edit is not installed (needed for 'cargo upgrade' and 'cargo set-version')."
    info  "Install it with:  cargo install cargo-edit"
    exit 1
fi

# ---------------------------------------------------------------------------
# Capture the pre-upgrade state so we can detect changes later
# ---------------------------------------------------------------------------
TOML_BEFORE=$(mktemp)
LOCK_BEFORE=$(mktemp)
cp Cargo.toml "$TOML_BEFORE"
cp Cargo.lock "$LOCK_BEFORE"
trap 'rm -f "$TOML_BEFORE" "$LOCK_BEFORE"' EXIT

OLD_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
info "Current app version: ${OLD_VERSION}"

# ---------------------------------------------------------------------------
step "1 — Check for outdated dependencies (dry-run)"
# ---------------------------------------------------------------------------
cargo update --dry-run 2>&1 || true

# ---------------------------------------------------------------------------
step "2 — Upgrade version constraints in Cargo.toml"
# ---------------------------------------------------------------------------
cargo upgrade --incompatible
info "Cargo.toml diff after upgrade:"
diff --unified=0 "$TOML_BEFORE" Cargo.toml || true

# ---------------------------------------------------------------------------
step "3 — Update Cargo.lock"
# ---------------------------------------------------------------------------
cargo update

# ---------------------------------------------------------------------------
# Detect whether anything actually changed
# ---------------------------------------------------------------------------
TOML_CHANGED=false
LOCK_CHANGED=false

if ! diff -q "$TOML_BEFORE" Cargo.toml &>/dev/null; then
    TOML_CHANGED=true
fi
if ! diff -q "$LOCK_BEFORE" Cargo.lock &>/dev/null; then
    LOCK_CHANGED=true
fi

if [[ "$TOML_CHANGED" == false && "$LOCK_CHANGED" == false ]]; then
    ok "All dependencies are already up to date. Nothing to do."
    exit 0
fi

# ---------------------------------------------------------------------------
step "4 — Verify build and tests"
# ---------------------------------------------------------------------------
info "Running cargo build …"
if ! cargo build; then
    error "Build failed after dependency upgrade. Fix the errors and re-run."
    exit 1
fi
ok "Build succeeded."

info "Running cargo test …"
if ! cargo test; then
    error "Tests failed after dependency upgrade. Fix the errors and re-run."
    exit 1
fi
ok "Tests passed."

# ---------------------------------------------------------------------------
step "5 — Bump app patch version"
# ---------------------------------------------------------------------------
cargo set-version --bump patch
NEW_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ok "Version bumped: ${OLD_VERSION} → ${NEW_VERSION}"

# ---------------------------------------------------------------------------
step "6 — Confirm release build"
# ---------------------------------------------------------------------------
info "Running cargo build --release …"
if ! cargo build --release; then
    error "Release build failed."
    exit 1
fi
ok "Release build succeeded."

# ---------------------------------------------------------------------------
step "7 — Summary"
# ---------------------------------------------------------------------------
printf "\n"
info "Cargo.toml dependency changes:"
diff --unified=0 "$TOML_BEFORE" Cargo.toml | grep '^[+-]' | grep -v '^[+-][+-][+-]' | grep -v '^[+-]version' || echo "  (no direct dependency version changes — only Cargo.lock transitive deps updated)"
printf "\n"
ok "App version: ${OLD_VERSION} → ${NEW_VERSION}"

# ---------------------------------------------------------------------------
step "8 — Push to GitHub"
# ---------------------------------------------------------------------------
TAG="v${NEW_VERSION}"

printf "\n"
info "The following will be committed, tagged, and pushed:"
info "  - Cargo.toml and Cargo.lock changes"
info "  - Tag: ${TAG}"
printf "\n"

read -rp "Commit, tag, and push to GitHub? [y/N] " REPLY
if [[ "$REPLY" =~ ^[Yy]$ ]]; then
    git add Cargo.toml Cargo.lock
    git commit -m "deps: upgrade dependencies → ${TAG}"
    git tag -a "$TAG" -m "Release ${TAG}"
    git push
    git push origin "$TAG"
    printf "\n"
    ok "Pushed commit and tag ${TAG} to GitHub."
else
    warn "Skipped. You can manually commit and push when ready:"
    info "  git add Cargo.toml Cargo.lock"
    info "  git commit -m \"deps: upgrade dependencies → ${TAG}\""
    info "  git tag -a ${TAG} -m \"Release ${TAG}\""
    info "  git push && git push origin ${TAG}"
fi

printf "\n%b%bAll done!%b\n" "$GREEN" "$BOLD" "$RESET"
