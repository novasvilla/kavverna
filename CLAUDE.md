# Kavverna

A KDE Plasma utility suite in Rust, with a Qt6/QML front end. It recreates the ideas of
[Vorssaint](https://github.com/vorssaintapp/vorssaint-utils), a macOS menu bar app, for
Linux. Fresh implementation, not a port. See CREDITS.md.

## Target

CachyOS (Arch), KDE Plasma 6.7, KWin on Wayland, Qt 6, Kirigami, PipeWire, Rust 1.85+ on edition
2024. Desktop machine with no battery, so battery features are out of scope. Two GPUs: an
NVIDIA discrete card read through NVML and an AMD integrated one read through sysfs. They
are never summed into one figure.

## Architecture

Crate boundaries enforce what would otherwise be convention.

```
domain/feature-catalog    Feature enum and descriptors: title, summary, group, icon, energy,
                          readiness, and the settings keys. Read by the features page and by
                          `settings.rs`. Depends on no feature crate.
domain/preferences        Settings store: JSON, atomic writes, 0700 dirs and 0600 files.
desktop/app-identity      What the desktop calls a process: desktop entries by binary, icon
                          and announced identity, Steam's game id, Electron's arguments.
desktop/kde-bridge        zbus proxies: KGlobalAccel, logind, ScreenSaver.
features/*                One crate per feature. No Qt, no display needed to test.
apps/kavverna-shell       The only crate that knows Qt. QObjects, QML, tray, D-Bus service.
```

Two rules follow from this and must hold:

1. **Only `apps/kavverna-shell` may depend on Qt.** Everything under `domain/`, `desktop/` and
   `features/` must compile and test with no display. If a crate needs Qt, the boundary is in
   the wrong place.
2. **`feature-catalog` may never depend on a feature crate.** That is what stops naming a
   feature from pulling its service into the binary.

### How a feature runs

Each one is a thread started from `main`, owning its work and publishing snapshots the shell
pushes into QML. `clipboard_state.rs` is the shape to aim for: it starts and stops with its
setting, so switching a feature off really does close what it had open. The mixer and the
sampler do not yet, and that is a gap rather than a choice.

**Availability sits above the enable keys.** `settings::is_installed` decides whether a feature
exists at all, and `main` never spawns a thread for one that is off. Removing a feature never
writes to its enable keys, so putting it back restores what it was configured to do. Anything
whose `Readiness` is `Planned` has no code behind it, can never be installed, and is listed only
so the catalogue stays honest about where the app is going.

A feature's enable key is named once, in the catalogue, and `settings.rs` reads it from there.
Writing it out a second time is how the switch and the thing it switches drift apart.

There was a `feature-runtime` crate with a reconciling registry, taken from the reference
application. Nothing ever used it, so it is gone; the two-layer model it carried now lives in
`settings.rs` and the features page, which is the smaller half that was actually wanted.

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
seams. Delete dead code rather than keeping it around. A comment describing behaviour that was
removed is worse than no comment, because the next reader believes it.

**Commits.** No AI attribution of any kind. No `Co-Authored-By` trailer, no generated-with
footer. Write the message as the author.

**Docs.** Everything in the repository is written in English. Plain prose, no em dashes, no
filler openers, no summarising conclusions. Describe what the software does, not what it
aspires to.

## Testing and release

`cargo test --workspace --exclude kavverna-shell` must pass, which is what CI runs. Anything
needing a live compositor, a live session bus or real sensors carries `#[ignore]` and says what
it needs, so plain `--workspace` picks up everything else. Run the rest with
`-- --include-ignored` on a desktop, and stop Kavverna first or the compositor tests write their
own copies into the real history.

**Build with `RUSTFLAGS="-D warnings"` before pushing.** CI sets it, the shell crate is not
linted by clippy there, and `cargo build` is the only thing standing between a dead-code warning
and a red main. A warning that is a yellow line locally is a failure there.

A discovery function that scans a system directory takes the root as a parameter, with a thin
wrapper passing the real one. `app_identity::process_in` and `Thermometer::discover_in` are the
shape. Fusing the path constant to the walk is what put a test on the machine's own hardware and
turned main red.

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
volume (a stream's on its node, a device's on its card route, with `save`), switching the
default output and input through the session's `default` metadata, cycling outputs, and
moving streams between devices.

Two contracts that were each learned the hard way, both by checking the system rather than
trusting our own state:

- **Moving a stream is a metadata write, and the number must be the serial.** Write key
  `target.object` on the `default` metadata with subject = the stream's node id and value =
  the target's `object.serial` (or its `node.name` as a string); `"-1"` means follow the
  default. WirePlumber matches a numeric value only against serials, so a node id written
  there fails silently to the default target, which is why the first attempt concluded that
  the property "does not work". Capture streams move by the identical write. `node.dont-move`
  streams ignore it entirely, and the row says so instead of hiding.
- **A device's volume and mute live on its card's route, not on its node's Props.** Writing
  the node changed a value nothing played through while every self-readback agreed; the USB
  microphone mute defect was the same wrong object. Every mixer write is verified with
  `pactl`, never with our own snapshot, and `tests/writes_to_pipewire.rs` keeps it that way.

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
repository. What a reader should see lives in the README, in `docs/` and in the What is next
section, which lists features and nothing else.

The history was rewritten once to take the file out of every commit, and the repository is
public at `github.com/novasvilla/kavverna`. Before any push that touches history again, check
that `git log --all --name-only -- ROADMAP.md` comes back empty.

Releases are tagged `vX.Y.Z` and the version lives in three places that move together: the
workspace `Cargo.toml`, `packaging/PKGBUILD` with a recomputed checksum, and `.SRCINFO`. The
fourth number a version shows is the CI run that built it, supplied by `KAVVERNA_BUILD`.

## When the panel does not open

A QML failure is silent. `QQmlApplicationEngine::load` reports nothing through the tracing
subscriber, so a broken file looks exactly like a broken tray icon: the process runs, the tray
answers, and nothing happens on a click. Run with `QT_LOGGING_RULES='qt.qml.*=true'` and the
real reason is at the end of the output. `panel.rs` also says so once, on the first request that
finds no interface attached.

The target is Linux with Plasma, and a design borrowed from elsewhere brings its platform with
it. `font.pixelSize` is an integer here, and a fractional size copied from a macOS layout cost a
whole session's debugging for one line. For anything that has an icon, draw it with
`Kirigami.Icon` from the desktop theme rather than a character, so it matches Breeze and cannot
land on a glyph the font does not carry.

**Every colour and every repeated dimension comes from `Theme.qml`.** It asks the desktop one
question, light or dark, and answers with Kavverna's own palette either way. Nothing writes a
colour at its use site. The stock Switch, CheckBox and Slider take the desktop's accent, so
`Toggle`, `Tick` and `Level` exist to wear the panel's instead; `PillButton` and `IconButton`
are the other two shared controls. Contrast was measured with every colour flattened onto what
sits behind it, since nearly all of them carry alpha and a translucent colour compared against
nothing is not what anybody sees.

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
