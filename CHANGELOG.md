# Changelog

Versions are `major.minor.fix.build`. The first three are the release; the fourth is the
build that produced the binary, stamped by CI and zero for anything built by hand.

## Unreleased

### Fixed

- **Dragging the panel carries the panel.** The gesture used to move an empty outline that
  froze a few pixels in: this compositor never repositions a mapped layer surface when only
  its margins change, which an isolated test surface proved. The moving picture now lives
  inside one still overlay, drawn at full strength with the real panel blanked for the
  gesture, so what follows the hand is the application itself, and it lands exactly where it
  shows because the overlay measures the screen's true free area, which no Qt property
  reports on Wayland. The shelf's drag shares all of it.
- **The mouse jiggle's timing and key rows are back on the panel's Tools page.** Moving them
  to settings read as tidy and lived as a loss; they now sit in both places, like the other
  duplicated quick controls. The longest-wait default became five minutes, a value its own
  choices can display.
- **Every page opens at its top.** The pages share one scroller, and a switch used to open
  the next page wherever the last one was left, first rows hidden above the fold.
- **The inside joke is findable.** The bubble hovers on the blue theme's own row and across
  the whole utilities row, not behind a 40 pixel switch.
- **Volume sliders answer the wheel**, two points a notch; the page keeps scrolling
  everywhere else.
- **A failed interface load names itself.** A missing QML runtime piece used to leave a live
  process behind a tray icon that opened nothing; it now prints four lines naming the cure
  and exits.

### Changed

- **The shelf's animation tells the whole story**: three new frames carry an item out under
  the copy cursor, through the compositor's own Move, Copy or Link menu, and off the shelf.
- **Both install routes name what they need.** The build-anywhere instructions listed no
  prerequisites at all; they now name the toolchain, headers and runtime modules, and point
  at `--selftest`. The package's dependency list was verified complete against an empty
  Arch container.

## 0.4.0

### Added

- **Each application can play through its own output.** The mixer row carries where the
  application plays; a tap opens every output inline, with Follow system default first. The
  choice is remembered by application and survives the device unplugging: sound falls back to
  the default meanwhile, the row says so, and the application returns to its device the moment
  it is plugged back in. Applications recording get the same against the microphones, and a
  stream that refuses to be moved keeps its row with the reason where the picker would be.
- **A shelf.** Files, text and links dropped onto it wait until they are dragged somewhere
  else. Dropping stages rather than copies: local files are held by path, only content with no
  file behind it is written into the shelf's own private directory, and a web address stays a
  link, never a download. One drop gesture forms one pile; drag out one item, a Ctrl+click
  selection, or the pile. A thin strip on the right screen edge opens the shelf the moment a
  drag touches it, Ctrl+Alt+S summons it even mid-drag, it can be dragged by its header to
  wherever it should live and reopens there, and it survives restarts behind a setting.
- **Two more themes, and a picker.** Torch stays the default and the look nothing changes
  without asking. Tide is the cave flooded in blue-slate; Ember is the cave burning down in
  red. Each is a designed whole with light and dark variants, measured against WCAG AA before
  the values froze, and the palette applies the moment a row is tapped.
- **The panel opens where it is useful.** Beside the tray icon by default, using the real
  coordinates the tray click carries on Plasma, on whichever screen and edge the bar lives; or
  wherever it was last dragged, one spot per screen, validated against the screens actually
  connected; or the old bottom right corner, byte for byte. Dragging the panel by its header
  shows an outline the exact size of the panel and places it on release.
- **The transformation card shows before it touches.** Plain, JSON and Markdown now preview
  the result with a sentence measuring it; the clipboard changes only on Use it, and the
  original copy stays in the history.
- **The settings page regrouped.** Placement and appearance first, the mouse jiggle's timing
  rows moved in from the panel where nobody could find them, startup sunk to the bottom.

### Fixed

- **Moving a stream between devices works now, and the old claim is corrected.** The first
  attempt wrote the stream's node id as the routing target and concluded the mechanism did not
  work; WirePlumber matches a numeric target only against the object's serial, so the write
  failed silently. The session now writes the serial, the same write pactl performs, and the
  live test reads the landing back from pactl by serial for playback and recording both.
- **Muting an application can no longer reach across the microphone.** Streams are grouped by
  role as well as by application, so an application's playback controls stop at its playback
  streams now that recording streams are tracked too.

## 0.3.0

### Added

- **Transform the clipboard.** What was copied can be made plain text, laid out JSON, or
  Markdown, from the clipboard page. The Markdown comes from the copy's own HTML, read at the
  moment you ask rather than stored, so the history keeps holding plain text only. It works on
  the copy that is still on the clipboard, the buttons follow what that copy offers, and every
  ask is answered in the panel, including not being JSON. This replaces the catalogued
  "Paste as plain text", which promised pasting, something Wayland does not allow without
  synthetic input; what this does is rewrite the selection so the next paste is the result.
- **A shortcut for every utility worth reaching blind**: the panel on Ctrl+Alt+K, keep awake on
  Ctrl+Alt+A, mute every microphone on Ctrl+Alt+M, the next output on Ctrl+Alt+O, beside the
  clipboard's Ctrl+Alt+V. Registered through the desktop as defaults only, so a key rebound in
  System Settings survives the next launch, and a utility switched off registers nothing.
- **The last two minutes behind each reading** in the monitor, drawn as a trace behind the bar,
  so the spike that ended before the panel opened is still there to see. Per graphics card, so
  switching cards shows that card's own past.
- **Come back to a preferred microphone.** Pin one in the settings and it is made the default
  again whenever it is plugged back in, and only then, so choosing another while it is here
  still works.
- **Step through the outputs you choose.** The switcher cycles the ticked set instead of every
  output there is. All of them, until you say otherwise.
- **Each utility says what it costs** in the settings list: nothing at rest, reads on a timer,
  watches the clipboard, waits on a key.
- **`--selftest`** prints one line for everything Kavverna relies on, names the distribution,
  and exits zero only when all of it answered, so a report from an untested machine arrives as
  data. An argument that is not understood is now said back with exit code 2 and the usage,
  instead of silently opening the panel.

### Fixed

- **The output volume slider did nothing, and never had.** Volume and mute were written into
  the node's Props, and for a hardware device the level that reaches the speaker lives on the
  card's route. Every readback of our own agreed with the write, which is why it went
  unnoticed: the first live test that read back with pactl instead found it within minutes.
  Writes go to the route now, with save set so a level survives a replug. The same fix ends
  the USB microphone refusing to mute, which had been in Known not to work since the start.
- **Middle clicking the tray held sleep off forever** whatever the default duration said. The
  toggle decision was written out four times and the fourth ignored the setting; it is decided
  in one place now.
- **Global shortcuts failed in silence.** The log filter named crates one by one and the crate
  that registers shortcuts was never on the list.

## 0.2.3

### Fixed

- **Restarting turned a timed hold into a permanent one.** Restore keep awake on start engaged
  the default duration rather than putting back what was running, so a thirty minute hold came
  back as one with no end at all, and a machine could stay awake indefinitely because Kavverna
  had been restarted. It now remembers what was held and puts back what is left of it. A hold
  that ran out while Kavverna was closed is over, and nothing is put back, since coming home to
  a machine that stayed awake all night is worse than losing the hold.
- **The default duration did nothing.** It said it was used by the switch and by auto start, and
  the switch and the tray menu both started a hold with no end whatever it was set to. All three
  read it now, which is what the setting always claimed.

## 0.2.2

### Added

- **An install URL that never changes.** Every release also carries the package under
  `kavverna-x86_64.pkg.tar.zst`, so
  `releases/latest/download/kavverna-x86_64.pkg.tar.zst` always points at the newest one and
  the instructions never go stale. pacman takes the version from inside the package rather than
  from the file name.

### Fixed

- **The documented way to install the published package did not work.** Handing `pacman` a URL
  makes it look for a signature beside the file, and the package is not signed, so it stopped
  with a 404 rather than installing. The README and the site now download the package and its
  published checksum, verify it, and install the local file. CI checks that the version in
  those instructions is the one being built.

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
