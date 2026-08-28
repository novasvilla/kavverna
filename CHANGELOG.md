# Changelog

Versions are `major.minor.fix.build`. The first three are the release; the fourth is the
build that produced the binary, stamped by CI and zero for anything built by hand.

## Unreleased

### Added

- Clipboard history over `ext_data_control_manager_v1`: text, images and files, with search
  through SQLite FTS5, pinning that survives both the size limit and a bulk clear, manual
  ordering and duplicate coalescing.
- A picker on Ctrl+Alt+V, registered through KGlobalAccel so it appears in System Settings.
  The search field takes focus, the arrows walk the list, and Enter or Ctrl+1 to Ctrl+9 put an
  entry back.
- Adopting Klipper's saved history, keeping the times and stars it already had.
- Emptying the clipboard on a timer, on suspend and on screen lock, working with the history
  switched off and never touching a saved entry.
- Taking campaign and click parameters out of copied links.
- An About section with the version, the licence and where settings are kept.
- Sound, monitoring, keep awake and the pointer nudge, each verified against the system rather
  than against the application's own state.
