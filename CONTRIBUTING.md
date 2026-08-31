# Contributing

More tools are welcome. The architecture is shaped so that adding one is a small, well bounded
job rather than a trek through the whole application.

## The shape of a feature

Three pieces, in this order:

1. **A crate under `features/`** that does the work and knows nothing about Qt. It should be
   testable with plain `cargo test`, without a display. If it needs Qt, the boundary is in the
   wrong place.
2. **A descriptor.** Add a variant to `Feature` in `domain/feature-catalog` and fill in its
   entry in `describe()`. Both are exhaustive matches with no wildcard arm, so the compiler
   tells you exactly what is missing, and the settings keys come from there rather than being
   written out twice. The catalogue depends on no feature crate: naming a feature can never
   drag its code in. A descriptor is `Readiness::Planned` until the crate exists, which lists
   it on the utilities page as on the way rather than offering a switch for nothing. Moving it
   to `Built` means editing the golden list in `tests/identity.rs`, deliberately.
3. **A section under `apps/kavverna-shell/qml/MenuPanel/`,** one file, plus a bridge object in
   `src/` that turns a snapshot into properties QML can read. A state module beside it owns the
   thread and publishes snapshots, following `clipboard_state.rs`. `main.rs` starts that thread
   only when the feature is installed, so a utility switched off on the settings page really
   does stop.

Two rules are not negotiable. Only `apps/kavverna-shell` may depend on Qt, and
`domain/feature-catalog` may never depend on a feature crate.

## Anything KDE specific

Reach it over D-Bus through `desktop/kde-bridge`, or through a QML import. KF6 cannot be linked
from this build: there are no pkg-config files for it and no CMake step here.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --all-targets --workspace --exclude kavverna-shell
cargo test --workspace --exclude kavverna-shell
RUSTFLAGS="-D warnings" cargo build --workspace
```

That last one matters more than it looks. CI turns warnings into errors, and the shell crate is
the one clippy does not see there, so `cargo build` is all that stands between a dead code
warning and a red main.

Tests that need a live compositor, a live session bus or real sensors carry `#[ignore]` and say
what they need, which is why the commands above pick up everything else. Run them with
`cargo test --workspace --exclude kavverna-shell -- --include-ignored` on a desktop. They take
the clipboard over and put back what they found, and they run one at a time. Stop Kavverna
first, or a running instance saves the tests' own copies into your history.

## House style

- Comments explain **why**, never what. If the code needs a sentence to say what it does, the
  names are wrong. No progress notes, no scaffolding vocabulary.
- Name things in the words a user of the feature would use. `manager`, `helper`, `utils` and
  `misc` are not names.
- Ship the simplest thing that solves the actual problem. No options nobody asked for.
- Verify against the system, not against the application's own state. Several bugs here passed
  their unit tests while doing nothing at all.
- English everywhere, in code, comments, commits and documentation.
- Every colour and repeated dimension comes from `qml/Theme.qml`. The stock Switch, CheckBox and
  Slider follow the desktop's accent rather than the panel's, so use `Toggle`, `Tick` and
  `Level` from `qml/Shared/` instead. Icons come from the desktop theme through `Kirigami.Icon`,
  never from a character.

## Adding a theme

A theme is one entry in the `palettes` object in `qml/Theme.qml`: the thirteen colour tokens,
each defined twice, `dark` and `light`. Copy an existing entry, change the values, and add a
row for it to the picker model in `qml/Settings/SettingsPage.qml`; nothing else refers to a
theme by name. Three rules keep a new theme worth shipping. Design the grounds and the inks,
not only the accent, or it reads as a tinted copy of the torch. Measure every text and surface
pair against WCAG AA before freezing values, the way the existing palettes were, and put the
ratios in the commit message; `mutedText` is decorative by design and is the one token allowed
below AA. And never change the torch, which is the look an absent setting has to keep meaning.

## Commit messages

A subject line saying what changed, then the reasoning: why this way, what was rejected, what
was measured. The history is the design record.
