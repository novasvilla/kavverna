# Kavverna

A single tray icon that does the job of a dozen small desktop utilities, for KDE Plasma on
Wayland.

## Why this exists

I use [Vorssaint](https://github.com/vorssaintapp/vorssaint-utils) every day on my Mac. It
bundles a per-app volume mixer, a system monitor, clipboard history, keep-awake and a lot
more behind one menu bar icon. When I moved to Linux on my desktop I could not find anything
that covered the same ground. Plasma ships pieces of it, spread across separate applets, and
none of them go as far.

Kavverna is my attempt at the missing tool. It is a fresh implementation in Rust, not a port.

## Status

Early. Nothing is released yet. The feature registry, the service lifecycle and the Qt/QML
shell build and pass their tests. Features land one phase at a time, and each one is used
daily on the author's machine before it is tagged.

Built and tested on CachyOS with KDE Plasma 6.7 on Wayland. Other distributions and desktops
are not supported yet. The architecture keeps all logic in plain Rust crates with the KDE
specifics isolated, so widening support later is a matter of adding backends rather than
rewriting.

## What Wayland does not allow

Being straight about this up front, because these limits are not going away:

- **Attributing a clipboard entry to the app that copied it.** Wayland carries no client
  identity on a selection, and KWin exposes no foreign-toplevel protocol. Kavverna filters
  password managers by mime type instead, and does not show a source column.
- **A shelf that follows the cursor mid-drag.** A client cannot observe a drag happening
  elsewhere. The shelf will anchor to a screen edge instead.
- **Managing other applications' windows.** No protocol for it on KWin.
- **True per-process power measurement.** RAPL needs root, so anything labelled energy use
  is an estimate and says so.

## Building

Requires Rust 1.85 or newer, Qt 6, and PipeWire development headers.

```sh
cargo build
cargo test
```

The build is pure Cargo. There is no CMake step.

## Credit

Kavverna recreates ideas from Vorssaint by studying what it does, not its source. See
[CREDITS.md](CREDITS.md).

## Licence

GPL-3.0-or-later.
