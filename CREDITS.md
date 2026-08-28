# Credits

## Vorssaint

Kavverna exists because of [Vorssaint](https://github.com/vorssaintapp/vorssaint-utils), a
macOS menu bar utility suite by the Vorssaint authors, licensed GPL-3.0-or-later.

Vorssaint is the reason this project has a shape at all. Its feature set defined what
Kavverna aims to cover, and two of its architectural decisions were adopted directly:

- **Two layer feature gating.** Whether a feature is installed is tracked separately from
  its own enable switches, so uninstalling a feature preserves how it was configured and
  reinstalling restores it.
- **Splitting pure description from live wiring.** Feature metadata lives somewhere that
  cannot reach a feature's implementation, so naming a feature can never start it.

Kavverna is an independent implementation. No Vorssaint code was copied or translated: it
targets a different platform, a different language and a different desktop, and the two
projects share no build system, dependency or runtime.

Two things were taken as data rather than written afresh, and it would be dishonest to imply
otherwise. The list of tracking parameters the link cleaner removes, and the per-site rules
that go with it, are the same names Vorssaint removes. They are facts about advertising
networks rather than an implementation, every one of them is public knowledge, and reinventing
the list would only mean getting it wrong for a while. The engine that applies them is our
own.

Kavverna is licensed GPL-3.0-or-later, matching Vorssaint, out of respect for the project
that inspired it.

## Platform work this builds on

- KDE Plasma and KWin, for the desktop this runs on
- PipeWire and WirePlumber, which make per-app audio routing possible
- [cxx-qt](https://github.com/KDAB/cxx-qt) by KDAB, for the Rust and Qt bridge
- [ksni](https://github.com/iovxw/ksni), for the StatusNotifierItem implementation
