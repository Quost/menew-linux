## What changes

<!-- What this PR does and why. Link the issue if there is one: "Closes #123". -->

## Screenshots

<!-- For visual changes: before and after, light and dark mode if it matters. -->

## Checklist

- [ ] Builds with `cargo build` and I tried the change in the app (GNOME or KDE, Wayland).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (and I added a test if I changed logic that has none).
- [ ] Every new UI string has en, pt, de, es and zh translations in `crates/menew-core/src/i18n.rs`.
- [ ] Screenshots regenerated if a screen changed (`bash scripts/snapshots.sh`, see CONTRIBUTING).
- [ ] `docs/parity.md` and the README are updated if the change is visible to users.
- [ ] `CHANGELOG.md` has a line for users under `## Unreleased`.
- [ ] Commits follow [Conventional Commits](https://www.conventionalcommits.org) (`feat(clipboard): ...`, `fix(palette): ...`).
- [ ] No telemetry, no new network calls and no new dependencies (or explained above).
