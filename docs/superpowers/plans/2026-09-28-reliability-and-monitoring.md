# Kavverna reliability and monitoring implementation plan

> **For agentic workers:** Continue this plan task by task. Keep the coverage ledger and verification evidence current before committing or pushing.

**Goal:** Review the whole current codebase, fix confirmed defects, add network and disk readings to the monitor, verify the complete experience on KDE/Wayland, and push the reviewed result to `origin/main`.

**Architecture:** Keep Linux reads in feature crates and Qt/QML only in `kavverna-shell`. Give each reading and persisted concept a domain name. Preserve existing settings, clipboard history, and shelf content across upgrades.

**Tech stack:** Rust 2024, Qt 6/QML, KDE Plasma 6 Wayland, PipeWire, zbus, SQLite, Arch packaging.

**Spec:** `CLAUDE.md`, `README.md`, `ROADMAP.md`, and the user request of 2026-09-28. Vorssaint is a behavior reference, not source to port.

## Global constraints

- Target: CachyOS/Arch, KDE Plasma 6.7, KWin Wayland, Rust 1.85+.
- Only `apps/kavverna-shell` depends on Qt; `domain/feature-catalog` depends on no feature crate.
- No network requests, telemetry, account, or background update check.
- Preserve `$XDG_DATA_HOME/kavverna/clipboard.db`, `clipboard-images/`, and `$XDG_CONFIG_HOME/kavverna/settings.json`.
- Use domain names, focused modules, and comments explaining why.
- Do not claim runtime success from compilation or unit tests alone.

## Coverage ledger

- [x] Read every Rust, QML, build, package, and CI file; record any exceptions here.
- [ ] Trace each user action from QML/tray/shortcut/D-Bus through Rust to the system or store and back to visible state.
- [x] Inspect privacy, permissions, data retention, migrations, error paths, concurrency, process execution, and Linux desktop integration.
- [ ] Classify findings as confirmed by a test, confirmed by source flow, or requiring a live reproduction.

## Review focus

- A duplicate Shelf deposit must remain usable after either copy is removed.
- A damaged or newer Shelf file must not cause loss of staged user content.
- A suspend-triggered clipboard clear must finish, or report a bounded failure, before its delay lock is released.
- Removing a feature must stop its owned work and release its runtime state without a restart. Shared workers stay alive only while another installed feature still needs them.
- Network and disk counters must handle first sample, reset, disappearance, and long gaps without false spikes.

## Task 1: Audit the current product

**Files:** All tracked Rust, QML, build, package, CI, and documentation files. Start with `features/`, `desktop/`, `domain/`, `apps/kavverna-shell/`, `.github/`, and `packaging/`.

- [x] Inventory the source files and assign review coverage without overlap.
- [x] Read each file and trace calls across crate and QML seams.
- [ ] Record confirmed findings with file, line, effect, and the smallest correction.
- [ ] Reconcile `ROADMAP.md` claims with the current code; correct stale claims in a separate documentation change.
- [ ] Run baseline formatting, workspace tests, Clippy, and warning-free build; record exact commands and results.

## Task 2: Protect Shelf content

**Files:** `features/shelf/src/lib.rs`, `features/shelf/src/staging.rs`, `features/shelf/src/store.rs`, and their focused tests.

- [x] Add a test that deposits identical text or images twice, removes one, and checks that the other is still alive and draggable. Confirm it fails first.
- [x] Keep a shared staged blob until no remaining item names it.
- [x] Add tests for interrupted or invalid saved JSON and a future version; preserve staged blobs on an unreadable file.
- [x] Write `shelf.json` atomically with private permissions and make staged files private from creation.
- [ ] Run Shelf tests, then an actual Shelf drop/restart/drag/remove flow on the desktop.
- [ ] Commit the self-contained change after fresh verification.

## Task 3: Make suspend clearing truthful

**Files:** `apps/kavverna-shell/src/auto_clear.rs`, `apps/kavverna-shell/src/clipboard_state.rs`, `features/clipboard-history/src/history.rs`, and `desktop/kde-bridge/src/session.rs`.

- [ ] Test the order between clear execution and delay-lock release, including a stalled or absent clipboard worker.
- [x] Acknowledge after a Wayland sync callback following the clear; bound waiting to two seconds and log failure.
- [ ] Verify clear on lock and suspend in a real KDE session without losing clipboard history.
- [ ] Commit after focused and live verification.

## Task 4: Give feature switches a real lifecycle

**Files:** `apps/kavverna-shell/src/features_view.rs`, `main.rs`, `mixer_state.rs`, `vitals_state.rs`, `clipboard_state.rs`, `shortcuts.rs`, and the features page QML.

- [x] Gate startup work by the installed features. The current restart notice is an interim, truthful state.
- [ ] Give every owned worker an explicit stop path and clear its snapshots after exit. Remove the `OnceLock` that prevents reconnecting the mixer.
- [ ] Reconcile feature selections after a successful settings write. Stop a shared worker only after its last installed consumer leaves; start it on the first consumer. Keep the panel, tray, shortcuts and QML state in agreement.
- [ ] Test on/off/on, rapid toggles, failed dependencies and shutdown for the mixer, monitor, clipboard, Keep Awake, auto-clear and Shelf. Check that disabled workers stop polling and close PipeWire, Wayland, D-Bus and filesystem handles as applicable.
- [ ] Measure `smaps_rollup`, thread count and open descriptors before/after disable and re-enable on the real desktop. RSS alone can retain freed allocator pages, so also verify worker exit and resource ownership.
- [ ] Replace the restart notice with immediate status only after those transitions pass live. Until then, the UI must keep telling the truth.
- [ ] Commit after fresh tests and runtime checks.

## Task 5: Add network readings

**Files:** `features/system-monitor/src/network.rs`, `lib.rs`, `apps/kavverna-shell/src/vitals_state.rs`, `vitals_view.rs`, `qml/MenuPanel/MonitoringSection.qml`, and the feature catalogue.

- [x] Test interface selection, counter deltas, resets, a long sample gap, hot unplug, and zero interfaces using fixtures.
- [x] Read per-interface counters locally and show upload/download rates and session totals with an honest first-sample state.
- [x] Show a useful empty/error state and clear units in the monitor; avoid an online speed test.
- [ ] Verify values against the same machine's kernel counters while traffic is generated locally.
- [ ] Commit the complete vertical slice.

## Task 6: Add disk readings

**Files:** `features/system-monitor/src/disk.rs`, `lib.rs`, `apps/kavverna-shell/src/vitals_state.rs`, `vitals_view.rs`, `qml/MenuPanel/MonitoringSection.qml`, and the catalogue if disk is a separately installable feature.

- [ ] Test mount and block-device identity, capacities, I/O deltas, resets, removable media, and unavailable counters with fixtures (mount and diskstats fixtures pass; delta/reset fixture still due).
- [x] Show capacity and local read/write rates for the relevant mounted volumes; do not imply that unavailable metrics are zero.
- [ ] Verify readouts against `findmnt`, `df`, and kernel counters on the real machine.
- [ ] Commit the complete vertical slice.

## Task 7: Independent review and delivery

**Files:** All changed production files, tests, docs, package metadata, and CI.

- [x] Have an independent adversarial reviewer compare the changed implementation with this plan and the reference behavior; fix confirmed gaps.
- [x] Run formatting, workspace tests, Clippy, a warning-free Rust build, a live monitor smoke test, and the relevant ignored compositor test after the last production edit.
- [ ] Verify no dead controls, stale notices, raw IDs, lost user data, privacy regressions, or unsupported claims.
- [ ] Review the diff, commit, push to `origin/main`, and compare local and remote SHAs and clean status.

## Task 8: Design third-party tools after the first-party lifecycle is proven

- [ ] Define a small versioned manifest with a stable feature id, title, icon, declared capabilities and executable. Reject duplicate ids, unsupported protocol versions and unsafe paths with a visible explanation.
- [ ] Launch each installed third-party tool as its own process only when enabled. Use a bounded local IPC protocol for state and actions; terminate it and release its memory on disable or shell exit. Keep Qt and Kavverna's private Rust ABI out of that contract.
- [ ] Define permissions for clipboard, audio, filesystem and notifications before exposing those capabilities. A manifest does not grant access by itself.
- [ ] Test crash, hang, restart, upgrade, disable and uninstall against a sample external tool. Keep existing first-party features built in until this contract has real users and measurements.

## Progress and evidence

### Review scope and findings

The source inventory contains 116 tracked Rust, QML, manifest, CI and package files. The Rust/Linux review read the feature crates for Shelf, Clipboard, Link Cleaner, Sound Mixer and System Monitor; the bridge, catalogue, preferences and shell; QML, build, CI and packaging paths. The root review traced the cross-crate seams and compared the private `ROADMAP.md` against actual features. `ROADMAP.md` is ignored and will remain outside the push. The new network and disk modules also received a read-through after implementation. An independent adversarial diff review found four additional gaps; each was corrected before final verification.

| Finding | Evidence | Resolution |
|---|---|---|
| Duplicate Shelf content deleted while still referenced | `shelf::remove` removed a BLAKE3 blob on the first removal; regression test failed before fix | Shared blob retained until the last item leaves. |
| Text bypassed Shelf's 16 MiB limit | `deposit` checked image bytes only; regression test failed before fix | Every staged incoming payload is counted before writing. |
| Damaged/future Shelf index could destroy user content | `load` silently returned empty and startup swept unreferenced blobs; regression test failed before fix | Invalid index is preserved, Shelf refuses mutations, JSON writes are atomic and private. |
| Suspend clear released logind delay early | The delay FD was dropped after enqueueing, ahead of the clipboard and Wayland threads | Wait for a bounded compositor sync acknowledgement; live compositor test checks the next paste is empty. |
| Feature switches hid utilities before resources changed | Services start once in `main`, while feature settings update immediately | Screen now says selection is saved for next start and highlights pending restart; network-only sampler avoids unrelated sensors. Immediate unload remains Task 4. |
| Clipboard privacy/UI drift | History switch used aggregate clipboard `wanted`; privacy text ignored link cleaning | Switch reflects history only, privacy text names the link-cleaning exception. |
| Imported secrets, encoded tracking names | Klipper import skipped the sensitivity rule; URL rules matched encoded query key bytes | Both use the same semantics as ordinary copies. |
| Corrupt settings, long list reads, malformed PipeWire metadata | Defaults could overwrite damaged JSON; SQL fetched full text for previews; metadata JSON was assembled by interpolation | Preserve invalid settings, fetch bounded previews, serialize/parse metadata as JSON. |

Remaining reviewed items for a later scoped change: lossless Linux paths through clipboard/Shelf wire formats, D-Bus methods reporting queued actions as success, and autostart desktop-entry escaping. They are recorded rather than silently folded into the monitoring feature.

### Verification so far

- Baseline: `cargo test --workspace --offline` passed; Shell Clippy had six warnings and the other crates were clean. The six Shell warnings have been fixed; Shell `cargo clippy -p kavverna-shell --all-targets --offline -- -D warnings` now passes. C++/Qt header diagnostics still appear from the toolchain.
- Shelf's 37 focused tests pass, including three tests that failed before the correction and a future-index protection case. The final `cargo test --workspace --offline -q` passes.
- `cargo fmt --all -- --check`, `git diff --check`, workspace Clippy with `-D warnings`, and `qmllint` on all four changed QML files pass.
- On KWin/Wayland, the ignored `a_clear_confirmation_follows_the_wayland_write` test passed after the compositor-sync change and observed an empty next paste. `--selftest` passed all session dependencies when run outside the tool sandbox.
- The live monitor probe found active `eno1`, `wlan0`, `tailscale0` and a system disk. The system disk's total and available bytes matched `df -B1 /`. Container links were then excluded from the visible network list and verified with a second probe.
- A KDE/Wayland capture of the new monitoring page showed the same three interfaces with changing rates and session totals, plus the system volume's free space and read/write rates. The test build used a temporary initial scroll position to expose those lower cards without sending input to the user's desktop; that QML change was removed, and the final source was rebuilt with `RUSTFLAGS="-D warnings" cargo build --workspace --offline -q`.
- The user is actively using Kavverna's Keep Awake. A temporary sleep inhibitor protected the live restart; the new build restored the hold, and D-Bus reported `Awake=true` with 3 h 58 min remaining. Do not close or restart it without a temporary inhibit and at least two hours restored in the next instance.
- The first manual launch lacked the KDE desktop environment, so themed toolbar icons were blank. Relaunching with the session's `XDG_CURRENT_DESKTOP`, `KDE_SESSION_VERSION` and `XDG_DATA_DIRS` restored Sound, Monitoring and Clipboard icons. The Tools tab is hidden by the user's existing `mouse-jiggle.installed=false` selection.
- The installed `/usr/bin/kavverna-shell` was restored as a user service after the UI test. D-Bus reported `Awake=true` and 3 h 46 min remaining after the temporary inhibitor was released.

Pending for this Git delivery: final diff review, commit, push and remote SHA comparison. A live Shelf drag/drop, actual suspend/lock transition and feature-switch transitions were not exercised while the user was working at the desktop; their focused tests and source review passed, but those full flows remain open acceptance checks. Immediate feature unload and third-party process integration are explicit follow-on tasks, with acceptance criteria above. A checked box requires evidence, not inference.
