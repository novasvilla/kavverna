# Changelog

Versions are `major.minor.fix.build`. The first three are the release; the fourth is the
build that produced the binary, stamped by CI and zero for anything built by hand.

## Unreleased

### Fixed

- **The documented way to install the published package did not work.** Handing `pacman` a URL
  makes it look for a signature beside the file, and the package is not signed, so it stopped
  with a 404 rather than installing. The README and the site now download the package and its
  published checksum, verify it, and install the local file. CI checks that the version in
  those instructions is the one being built, since the same number now lives in four places.

## 0.2.1

### Fixed

- **Switching off one utility on a page that hosts several did nothing visible.** Turning the
  volume mixer off left its rows on the sound page, because the gating stopped at the page and
  the page stays while any of the three sound utilities is installed. System monitor looked
  fine only because its page holds one utility and vanished whole. Each card and each settings
  row now answers for the utility that owns it.
- A release carried a debug package beside the real one. What makepkg splits out is decided by
  the machine it runs on, and that is pinned in the package definition now, like the link time
  optimisation option beside it.

## 0.2.0

### Added

- **Every utility has its own switch.** A page listing all thirteen, grouped as the panel
  groups them. One switched off disappears from the panel, the tab strip and the rest of the
  settings, and stops running: its thread never starts. Availability sits above each feature's
  own setting, so removing one never writes to it and putting it back restores what it was
  configured to do. The four that are catalogued and not written yet say so instead of
  offering a switch for nothing.
- **A light theme, and a switch between them.** The panel follows the desktop into light or
  dark, or can be pinned to either. The colours are Kavverna's own in both, charcoal and stone
  one way and warm parchment the other, with torchlight amber the accent throughout.
- **A tray menu that reaches the whole suite.** Muting every microphone, moving to the next
  output and opening the history, alongside keep awake, and showing only what is installed.
- **Application icons in the mixer**, drawn from the desktop entry that named the row.
- **Muting an output and choosing an input**, both of which the backend had always accepted
  with nothing on screen to ask for them.
- **A built package on every release.** Each release now carries an
  `x86_64.pkg.tar.zst` beside the source tarball, so installing on Arch or CachyOS is
  `sudo pacman -U` against the attached file. It is produced by `packaging/PKGBUILD`
  from the tagged commit, so it resolves exactly the dependencies a source build does.
  Building it yourself with `makepkg -si` stays the recommended route on a machine you
  keep, since that links against the libraries the machine has rather than the ones
  Arch shipped on the day of the release.
- `kavverna-shell --version` and `--help`. Both used to open the panel in silence.

### Fixed

- **A program is found by the name it announces, not only by the binary it runs.** Vesktop
  showed as lowercase vesktop with a generic mark, because it reports `electron` as its binary
  and no desktop entry runs a program by that name. Every Electron application landed in the
  same place. Entries are now indexed by `StartupWMClass` and by their own file name too, which
  is what those exist for.
- **The secret guess sees the shapes secrets actually take.** A JSON web token ran past the
  length it worked within, a key block was rejected for containing line breaks, and a connection
  string only tripped it when the password happened to contain a digit. All three are recognised
  by shape now. A plain connection string with no secret in it is no longer dropped either,
  since only http and https had counted as links.
- **Clearing the clipboard on suspend no longer races it.** It holds a logind delay lock while
  it works and lets go once the clipboard has actually been emptied. The lock is only taken
  while the setting is on.
- **The light theme is readable.** Nine buttons drew their own background and left their label
  to the desktop's palette, so on a light panel under a dark desktop the text came out white on
  parchment.
- **The energy and monitoring marks mean something.** Tinting an icon to one colour keeps its
  outline and discards everything inside it, which turned a chart in a frame into a plain square
  and a bolt in a disc into a plain circle.

### Internal

- Tests no longer depend on the machine that runs them. A discovery function takes its root, so
  the thermal test reads a tree it wrote rather than the sensors of whatever built it, which is
  what had turned main red.
- CI stopped selecting with `--lib`, which had been excluding six tests that pass anywhere,
  among them the one guarding persisted feature ids against a rename that would orphan a user's
  settings. Anything needing a live desktop carries `#[ignore]` and says what it needs.
- Two compositor tests asserted only that nothing arrived, so a watcher that died at startup
  passed them. Each now proves it was listening.
- Releases no longer build without running the suite first.

## 0.1.5

### Fixed

- **A game showed up as SDL Application.** The mixer now asks the desktop what a program is
  called before believing what its toolkit calls itself. A Steam game is matched through the
  identity Steam hands it and the icon Steam names in the entry it writes, so every game is
  covered rather than the ones somebody thought of; anything else is matched by the binary its
  desktop entry runs.

## 0.1.4

### Fixed

- **The settings page ran off the side.** The scrolling area bound its content to the
  flickable's width rather than to what is visible, so once a scrollbar appeared the right hand
  edge of every row went past it.
- The settings file path was printed twice, once loose in the middle of the energy section.

## 0.1.3

### Fixed

- **The settings page was the height of the screen.** Every page now keeps to the height the
  rest of them use and anything longer scrolls, rather than growing into a column nobody can
  read.
- **The pointer nudge looked unavailable on a working machine.** It was looking for ydotool's
  socket in `/tmp` only; `ydotoold` puts it in the runtime directory when it has one.

### Added

- Screenshots, and install instructions that do not need the AUR.

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
