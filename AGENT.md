# Agent Instructions

## Triggers

| Phrase | Workflow |
|---|---|
| "Update all deps" | [Update All Dependencies](#update-all-dependencies) |

---

## Update All Dependencies

Bump every dependency in `Cargo.toml` to its latest compatible version, update `Cargo.lock`, and increment the app's patch version.

### Prerequisites

`cargo-edit` must be available for `cargo upgrade`. Install it if it is missing:

```sh
cargo install cargo-edit
```

### Steps

#### 1. Check for outdated dependencies

Run the following to see what is stale before making any changes:

```sh
cargo update --dry-run 2>&1 || true
```

This gives a human-readable picture of what `cargo update` would do (lock file level). Note the output but do not stop here — we want to upgrade beyond lock-file pinning as well.

#### 2. Upgrade version constraints in `Cargo.toml`

`cargo update` only moves within the version ranges already written in `Cargo.toml`. To go further, use `cargo upgrade` (from `cargo-edit`) to rewrite the version specifiers in `Cargo.toml` to the latest available for every dependency:

```sh
cargo upgrade --incompatible
```

- `--incompatible` allows upgrades that cross a semver-major boundary (e.g. `0.11` → `0.12`, `1.x` → `2.x`).
- After this step, inspect the diff in `Cargo.toml` to confirm the changes look correct.

#### 3. Update `Cargo.lock`

Regenerate the lock file to match the new `Cargo.toml` constraints:

```sh
cargo update
```

#### 4. Verify the project still compiles and tests pass

```sh
cargo build --verbose
cargo test --verbose
```

If there are compile errors caused by breaking API changes in an upgraded crate, fix the code in `src/` to match the new API before proceeding. Do **not** roll back the dependency version unless the breakage cannot be resolved.

#### 5. Bump the app version in `Cargo.toml`

Read the current `version` field from `[package]` in `Cargo.toml`, increment the **patch** segment by 1, and write it back.

For example, if the current version is `0.4.3`, write `0.4.4`.

Use `cargo-edit` for this:

```sh
cargo set-version --bump patch
```

#### 6. Confirm the final state

```sh
cargo build --release --verbose
```

A successful release build confirms everything is consistent.

#### 7. Report what changed

Summarise the results in a short message covering:

- Which dependencies were upgraded and from what version to what version (diff of `Cargo.toml`).
- The new app version.
- Whether any source-code changes in `src/` were required to fix breaking API changes.

---

### Notes

- Always run `cargo build` and `cargo test` before bumping the app version. A version bump should only land on a green build.
- If `cargo upgrade --incompatible` causes too many breaking changes at once, upgrade crates one at a time and fix compilation errors incrementally.
- Do not modify `Cargo.lock` by hand — always let Cargo manage it.
