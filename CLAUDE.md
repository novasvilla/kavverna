# Kavverna

A KDE Plasma utility suite in Rust, with a Qt6/QML front end. It recreates the ideas of
[Vorssaint](https://github.com/vorssaintapp/vorssaint-utils), a macOS menu bar app, for
Linux. Fresh implementation, not a port. See CREDITS.md.

## Target

CachyOS (Arch), KDE Plasma 6.7, KWin on Wayland, Qt 6.11, PipeWire, Rust 1.85+ on edition
2024. Desktop machine with no battery, so battery features are out of scope. Two GPUs: an
NVIDIA discrete card read through NVML and an AMD integrated one read through sysfs. They
are never summed into one figure.

## Architecture

Crate boundaries enforce what would otherwise be convention.

```
domain/feature-catalog    Feature enum and descriptors. Depends on no feature crate.
domain/feature-runtime    Service traits and the reconciling registry.
domain/feature-assembly   The only crate that depends on every feature. Builds services.
domain/preferences        Settings store, registered defaults, migrations, backup.
domain/private-store      XDG file storage, 0700 dirs and 0600 files.
domain/shortcut-registry  Shortcut roles behind a swappable backend.
desktop/kde-bridge        Every zbus proxy: logind, ScreenSaver, Solid, notifications.
features/*                One crate per feature. No Qt.
apps/kavverna-shell       The only crate that knows Qt. QObjects, QML, tray.
panel/                    Plasma applet for live readouts, reads over D-Bus.
```

Two rules follow from this and must hold:

1. **Only `apps/kavverna-shell` may depend on Qt.** Everything under `domain/` and
   `features/` must compile and test with no display. If a crate needs Qt, the boundary is
   in the wrong place.
2. **`feature-catalog` may never depend on a feature crate.** That is what stops naming a
   feature from pulling its service into the binary.

### Feature lifecycle

Availability ("installed") sits above each feature's own enable keys. Uninstalling clears
only availability, so reinstalling restores prior configuration.

A service only knows how to start. `FeatureRegistry::reconcile` decides whether it should be
running, and presence in the registry is what running means. Reconcile is idempotent, so it
can be called after any settings change. Adding a `Feature` variant fails the build in
`describe` and in the assembly, both of which are exhaustive `match` with no wildcard arm.

## Conventions

**Naming.** Domain language only. Names say what a thing is in the product, never when it
was built or how. Banned as identifiers: `utils`, `helpers`, `core`, `common`, `manager`,
`misc`, `data`, and anything carrying a phase, wave or spike number. Scaffolding vocabulary
must not survive into committed code.

**Comments.** Explain why, never what. A comment earns its place when it records a
non-obvious decision, an invariant, a hazard, or a constraint imposed from outside. Do not
narrate code that already reads clearly, do not restate signatures, do not leave progress
notes or changelog entries in comments.

**Simplicity.** Build what the current phase needs. No speculative config knobs or plugin
seams. Delete dead code rather than keeping it around.

**Commits.** No AI attribution of any kind. No `Co-Authored-By` trailer, no generated-with
footer. Write the message as the author.

**Docs.** Everything in the repository is written in English. Plain prose, no em dashes, no
filler openers, no summarising conclusions. Describe what the software does, not what it
aspires to.

## Testing and release

`cargo test --workspace` must pass, and the `domain/` and `features/` crates must pass with
no display available.

Tests are not enough on their own. A feature is done when it has been run in the real
desktop and checked by hand: the mixer against two apps playing at once, the monitor against
`nvidia-smi`, `sensors` and `free -h` in parallel, keep-awake against `systemd-inhibit
--list`.

Nothing is tagged or packaged until it has been in daily use on the author's machine. The
repository is public from the start; releases are what is held back.

## Hazards

Fan control writes PWM values to `/sys/class/hwmon/*` as root. A fan left at 0 RPM can
damage hardware. It is deferred to a late phase and, when it lands, must run as a separate
privileged daemon with a heartbeat that restores automatic mode when the UI stops answering,
read-back verification after every write (the `nct6799` chip ignores writes silently when
`pwm_enable` is wrong), an independent thermal watchdog, and a duty floor that never accepts
zero. Automatic mode is the state to fail toward in every path.
