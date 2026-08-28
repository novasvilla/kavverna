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

## Calling into Rust from QML

An `#[qinvokable]` keeps the name it was given, so `toggle_awake` is `toggle_awake` in QML.
A `#[qproperty]` does not: it generates camelCase accessors, so `page` becomes `getPage` and
`setPage`. Calling `set_page` from QML therefore raises a TypeError and the whole handler is
abandoned silently, which reads as a dead control rather than an error.

Write to a property by assigning it (`hub.page = 2`), never by calling a setter. Reserve
method calls for functions actually declared `#[qinvokable]`. When a control does nothing,
read the generated header under `target/debug/build/*/out/cxxqtbuild/include/` and check what
the member is really called before assuming anything.

## What blocks sleep on KDE

PowerDevil, not logind, decides when a KDE session suspends. `AutoSuspendIdleTimeoutSec` in
`powerdevilrc` is the timeout it acts on.

Both routes work and PowerDevil honours each: an inhibition through
`org.kde.Solid.PowerManagement.PolicyAgent.AddInhibition`, and a plain logind `idle` block
inhibitor, which PowerDevil picks up as well. Kavverna asks the policy agent because that is
explicit and because it can also request `ChangeScreenSettings` to hold the display on,
which logind has no way to express. The logind inhibitor stays as a fallback for sessions
with no desktop power daemon, and is skipped when the policy agent answers, since holding
both registers the same hold twice.

Verify with `ListInhibitions` on the policy agent, but note two traps. It is marked
deprecated and lags the actual registration by several seconds, so a read straight after the
call reports nothing and proves nothing. And a quiet moment where it returns an empty list
says only that nothing is inhibiting right now. `AddInhibition` returning a cookie is the
immediate, authoritative answer.

## Sound, verified and unverified

Volume is amplitude cubed: a slider at 50 reads 0.125 in the graph. Measured against a live
sink, not assumed.

Identifying the application behind a stream needs three passes. The registry hands out fewer
properties than the bound object does, and a node usually reaches the registry before the
client that owns it, so a stream is identified again when its client appears and again when
its own info event arrives. Where only a process id is known the binary comes from /proc,
skipping the audio server's own bridges: a stream arriving through the PulseAudio bridge
reports the bridge rather than the application.

Working and verified against the live system: reading every device and stream, writing
volume, switching the default output and input through the session's `default` metadata, and
cycling outputs.

Two open questions, both found by checking the system rather than trusting our own state:

- **Per application routing does not work by setting a property.** Neither `target.object`
  nor `target.node` moves a stream that is already playing, confirmed by watching the Link
  objects rather than the metadata write. `pactl move-sink-input` does move it, so a live
  move needs the links rebuilt.
- **Muting a USB headset microphone reports success but `pactl` still shows it unmuted.**
  The likely cause is that a node's mute and a device route's mute are different layers, and
  each tool reads a different one. Not confirmed. Until it is, do not claim mute-all covers
  every input.

## Hazards

Fan control writes PWM values to `/sys/class/hwmon/*` as root. A fan left at 0 RPM can
damage hardware. It is deferred to a late phase and, when it lands, must run as a separate
privileged daemon with a heartbeat that restores automatic mode when the UI stops answering,
read-back verification after every write (the `nct6799` chip ignores writes silently when
`pwm_enable` is wrong), an independent thermal watchdog, and a duty floor that never accepts
zero. Automatic mode is the state to fail toward in every path.

## Publishing

`ROADMAP.md` is a local working file. It is in `.gitignore` and must never be committed: it
carries half-formed ideas, money questions and notes to self that have no place in a public
repository. The plan a reader is meant to see lives in the README and in `docs/`.

Earlier commits still contain it. The repository has no remote and has never been pushed, so
going public means creating the GitHub repository and pushing **one clean commit with no
history**, not this local history. Check `git log --all --name-only -- ROADMAP.md` comes back
empty on whatever is about to be pushed.

## When the panel does not open

A QML failure is silent. `QQmlApplicationEngine::load` reports nothing through the tracing
subscriber, so a broken file looks exactly like a broken tray icon: the process runs, the tray
answers, and nothing happens on a click. Run with `QT_LOGGING_RULES='qt.qml.*=true'` and the
real reason is at the end of the output. `panel.rs` also says so once, on the first request that
finds no interface attached.

The target is Linux with Plasma, not macOS. Read the reference app for behaviour and for how it
organises files, never for values: `font.pixelSize` is an integer here, and copying its 11.5 and
10.5 across cost a whole session's worth of debugging for one line. For anything that has an
icon, use `icon.name` from the desktop theme rather than a character, so it matches Breeze and
cannot land on a glyph the font does not carry.

Running the test suite while Kavverna is running writes the tests' own copies into the real
clipboard history. Stop the app first, or clear the history afterwards.

## What has to survive an upgrade

Two things on disk belong to the user and must never be lost by a new build:

- `$XDG_DATA_HOME/kavverna/clipboard.db` and `clipboard-images/`, everything they copied.
- `$XDG_CONFIG_HOME/kavverna/settings.json`, every choice they made.

Changing the database shape means raising `SCHEMA_VERSION` in `store.rs` and adding a step to
`migrate`, which applies only what a given file is missing. A file written by a newer build is
left alone rather than rewritten: every read names its columns, so an unknown newer one is
ignored instead of destroying anything. There are tests for reopening, for a database from
before versioning, and for a pinned entry surviving both.

Renaming a settings key or a feature id orphans what the user had set. Neither is worth doing
after a release without a migration for it.
