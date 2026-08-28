# Changelog

Versions are `major.minor.fix.build`. The first three are the release; the fourth is the
build that produced the binary, stamped by CI and zero for anything built by hand.

## 0.1.2

### Fixed

- **Keep awake could hold a machine awake with nobody holding it.** A failure after the power
  daemon had accepted the inhibition left it registered with no cookie anywhere to release it.
- **The pointer nudge called itself available whenever the ydotool binary existed.** It needs
  the daemon and a socket too, or every nudge fails while the switch looks as though it worked.
- **The tray was asked for once.** Started from autostart, the panel that hosts it is often not
  up yet, which left the application running with no way to reach it.

### Removed

- `feature-runtime`, a reconciling registry nothing used. `feature-catalog` earns its place
  instead: a feature's enable key is named there once and the settings module reads it from
  there, so the switch and the thing it switches cannot drift apart.

## 0.1.1

Six independent reviewers went over the code before it had been out an hour. These are what
survived a second pass that tried to refute them.

### Fixed

- **Switching the history off did not stop copies reaching the disk.** With link cleaning on,
  the content had to be read to be rewritten, and it was then saved anyway: the flag gated the
  read and not the write. The privacy page promised otherwise. There is now a test for it.
- **The clipboard database and its images were world readable**, which the privacy page also
  said they were not. Both are `0600` inside a `0700` directory now, with a test that checks
  the mode rather than the intention.
- **The pointer nudge had no working settings.** Every control on the Tools page was bound to a
  property the bridge stopped having, so nothing highlighted and every tap threw an error QML
  swallows. All four real settings have controls now, and they say what the jiggler does: a
  wait drawn afresh between two bounds rather than a fixed interval.
- **A password copied from a password manager never started the clear timer.** Its content is
  still never read, but the copy is noticed, which is what the timer needs.
- **Copied text was rendered as markup** in the panel, so a copied string that looked like
  HTML would be laid out and anything it referenced fetched. It is plain text now.
- **The panel could grow past the screen.** The pages scroll and the chrome stays put.
- **Every launch put its own shortcut back**, overwriting one changed in System Settings. The
  key it registers is a default now, and the desktop's own store wins.
- Security and credit pages corrected: the pointer nudge does use synthetic input through
  `ydotool`, and the tracking parameter lists come from the reference app as data.

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
