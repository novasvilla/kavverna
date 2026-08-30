# The clipboard on KDE Plasma

What a clipboard manager can and cannot do on this desktop, what Kavverna does about each, and
what is still to come. The verdicts here were established against a running KWin rather than
read from a specification.

## What this machine offers

Checked with `wayland-info` on KWin 6.7.4:

| Protocol | State | Consequence |
|---|---|---|
| `ext_data_control_manager_v1` v1 | present | Capture is event-driven. No polling. |
| `zwp_primary_selection_device_manager_v1` v1 | present | The middle-click selection can have its own history. |
| `ext_idle_notifier_v1` v2 | present | Idle-based clearing works, with an inhibitor-aware variant. |
| `zwlr_layer_shell_v1` v5 | present | The picker can be an edge-anchored surface with keyboard focus. |
| `zwp_input_method_v1` v1 | present, slot free | Snippet expansion without keylogger-grade access. |
| `zwp_virtual_keyboard_manager_v1` | absent | `wtype` fails. Synthetic paste needs ydotool. |
| `zwlr_foreign_toplevel_manager_v1` | absent | The application that copied cannot be identified. |
| SQLite 3.53 with FTS5 | present, with pkg-config | Search comes from the system library, nothing bundled. |

## What is built

**Capture.** Text, images and file lists, over `ext_data_control_manager_v1`. The compositor
reports a change rather than being polled for one, so nothing is read that nobody copied.

**Storage.** SQLite with an FTS5 index beside it. Rows come back as previews and the full text
is fetched only for the entry being looked at, so ten thousand rows cost a page rather than
everything ever copied. Images are files named by their digest, so the same image copied twice
is stored once and a deleted row takes its picture with it. The database and its images are
`0600` inside a `0700` directory.

**The list.** Search, pinning, manual ordering within a group, duplicate coalescing, a size
limit that pinned entries do not count toward, and a bulk clear that never removes anything
pinned.

**The picker.** Ctrl+Alt+V from anywhere, registered through KGlobalAccel so it appears in
System Settings beside every other shortcut. The search field takes focus, the arrows walk the
list, and Enter or Ctrl+1 to Ctrl+9 put an entry back.

**Exclusions.** A copy carrying `x-kde-passwordManagerHint` is never read at all, so nothing
sensitive reaches the process. Text shaped like a secret is left out separately, and that one is
a guess that errs toward dropping a copy too many. Three of its rules match a shape rather than a
length: a JSON web token by its three dotted parts, since those run past the ceiling the general
rule works within; a key block by its `-----BEGIN` line, since it is mostly line breaks and the
general rule rejects anything carrying one; and a URL of any scheme holding a user name or
password, which is what a connection string is.

**Auto clear.** On a timer, on suspend through logind, and on screen lock. Independent of each
other, working with the history switched off, and never touching a saved entry. The suspend one
holds a logind delay lock while it works, so the machine waits for the clipboard to be emptied
rather than racing it. `delay` and never `block`: refusing a suspend outright is not what this
is for, and the lock is only taken while the setting is on.

**Link cleaning.** Campaign and click parameters removed the moment a link arrives, with the
rest of the query left byte for byte as it was. Refused outright when the copy carries anything
richer than plain text, since taking the selection destroys what its owner offered.

**Taking over from Plasma.** Klipper's saved history can be adopted, keeping the times and stars
it already had.

**Transformation.** What is on the clipboard right now can be re-offered as plain text, laid
out as JSON, or turned into Markdown. The Markdown comes from the copy's own `text/html`, read
at the moment of the ask rather than stored: the watcher keeps the current offer alive, and an
offer stays readable until the next one replaces it, which a probe against the running KWin
established. So the history still holds plain text only, and the richer type never touches
disk. It works only for the copy that is still on the clipboard, which is the honest limit, and
the buttons follow what the current offer holds rather than failing.

## What Wayland does not allow

These are limits of the protocols, not of the implementation, and they are not going away.

**Knowing which application copied something.** A selection carries no client identity, and KWin
advertises no foreign toplevel protocol. The only non-interactive route is injecting a script
into the compositor to report the focused window, which is focus correlation rather than
attribution. So there is no source column, and exclusions work by mime type instead.

**Emptying the clipboard while Plasma's own is running.** Klipper puts the content straight back
whenever anything empties the selection, and marks what it re-asserts with
`application/x-kde-onlyReplaceEmpty`. Kavverna ignores that marker so the two stop fighting, but
nothing is visibly emptied until Prevent empty clipboard is turned off in System Settings.

**Recognising a link by its type.** There is no single convention. A Chromium offer carries only
`text/plain` and a private source type; Firefox offers UTF-16 `text/x-moz-url`. So a link is
found by looking at the text, which will occasionally promote a string somebody meant as text.

**Closing the picker with the click that lands elsewhere.** To see a click outside a window you
must own the output's input region, which consumes the click. Dismiss-then-click becomes two
clicks, and there is no way around it with the protocols here.

**Pasting into the application you came from.** ydotool injects kernel keycodes, so it depends
on the keyboard layout, and its daemon needs `input` group membership, which makes every input
device readable by any process running as you. That cost is why nothing here requires it yet.

**Restoring the clipboard after a temporary write.** Taking the selection destroys the previous
owner's offer permanently, so only bytes drained beforehand can be replayed. Any lazily rendered
or application-private type is gone.

**A shelf that follows the cursor.** A client cannot be told where the pointer is, and cannot
position its own window. An edge-anchored shelf is the honest version.

**Detecting that the displays went to sleep.** PowerDevil publishes no such signal and KWin
exposes no DPMS path. Approximating it from an idle timer drifts whenever the power profile
changes, so that trigger is not offered rather than faked.

## What Linux allows that macOS does not

**A second clipboard.** The middle-click selection has no counterpart on macOS.
`ext_data_control_device_v1` reports it alongside the ordinary selection, so it can have its own
history, and copying between the two becomes a choice rather than an accident.

**A launcher that is already there.** KRunner takes a D-Bus plugin, so the history and the
snippets can be reached from the launcher people already use, instead of another overlay window
with its own fuzzy search.

**Naming an application by what the desktop calls it.** Every installed program has an entry
with the name a person uses and the icon to go with it. That is how a stream announcing itself
as SDL Application becomes the game it actually is.

## Still to come

The public list lives in the README's "What is next" and is not repeated here. One idea is
recorded here because it belongs to this document: **a rules editor for link cleaning**, so the
built-in list can be added to and switched off without editing a file.

## Credit

The ideas here come from a macOS application, studied for what it does rather than for how it
does it. See [CREDITS.md](../CREDITS.md).
