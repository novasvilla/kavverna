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
   drag its code in.
3. **A section under `apps/kavverna-shell/qml/MenuPanel/`,** one file, plus a bridge object in
   `src/` that turns a snapshot into properties QML can read. A state module beside it owns the
   thread and publishes snapshots, following `clipboard_state.rs`.

Two rules are not negotiable. Only `apps/kavverna-shell` may depend on Qt, and
`domain/feature-catalog` may never depend on a feature crate.

## Anything KDE specific

Reach it over D-Bus through `desktop/kde-bridge`, or through a QML import. KF6 cannot be linked
from this build: there are no pkg-config files for it and no CMake step here.

## Before you open a pull request

```sh
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The tests under `features/*/tests` run against the live desktop rather than a fake, because
every interesting failure in this application is in the protocol rather than in the logic. They
take the clipboard over and put back what they found, and they run one at a time. Stop Kavverna
before running them, or clear your history afterwards, since a running instance will save the
tests' own copies.

## House style

- Comments explain **why**, never what. If the code needs a sentence to say what it does, the
  names are wrong. No progress notes, no scaffolding vocabulary.
- Name things in the words a user of the feature would use. `manager`, `helper`, `utils` and
  `misc` are not names.
- Ship the simplest thing that solves the actual problem. No options nobody asked for.
- Verify against the system, not against the application's own state. Several bugs here passed
  their unit tests while doing nothing at all.
- English everywhere, in code, comments, commits and documentation.

## Commit messages

A subject line saying what changed, then the reasoning: why this way, what was rejected, what
was measured. The history is the design record.
