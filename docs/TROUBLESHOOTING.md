# Troubleshooting

## The tray icon is there but clicking it does nothing

The interface failed to load and Qt reports that through its own logging rather than through
the application's. Run it with the QML channel on and the reason is at the end of the output:

```sh
QT_LOGGING_RULES='qt.qml.*=true' kavverna-shell
```

## A page or a whole section is missing

Every utility has a switch on the settings page, and one that is off is hidden from the panel,
from the tab strip and from the rest of the settings. Turning it back on restores whatever it
was configured to do, since removing one never writes to its own settings. Some entries there
say **On the way** instead of carrying a switch, which means the utility is catalogued and not
written yet.

## An application in the mixer has the wrong name or no icon

Kavverna asks the desktop what a program is called rather than believing what its toolkit says,
and it tries three ways in: the identity Steam hands a game, the identity the program announces
through `StartupWMClass` or its own `.desktop` file name, and the binary it runs. A program with
no desktop entry at all has nothing to be looked up in, and shows the name its toolkit reports
with the generic icon. Check what it is announcing:

```sh
pw-dump | grep -A2 'application.name'
```

## The clipboard never empties itself

Plasma's own clipboard puts the content straight back whenever anything empties the selection,
and marks what it re-asserts with the mime type `application/x-kde-onlyReplaceEmpty`. Turn off
**Prevent empty clipboard** in System Settings under Clipboard. It may take a new session.

## Two clipboard histories

Klipper is still running. Adopt its history from the Kavverna settings page, then turn Plasma's
own clipboard off in System Settings so the two stop competing.

## The panel opens on the wrong screen

It anchors to the screen the window is on rather than to whichever screen has focus, so a
fullscreen window on another monitor cannot drag it away. If it is on the wrong one, the tray
icon is on that screen's panel.

## Keep awake seems to do nothing

Check what the system thinks:

```sh
systemd-inhibit --list
busctl --user call org.kde.Solid.PowerManagement.PolicyAgent \
  /org/kde/Solid/PowerManagement/PolicyAgent \
  org.kde.Solid.PowerManagement.PolicyAgent ListInhibitions
```

`ListInhibitions` is deprecated and lags several seconds behind the actual registration, so a
read straight after switching it on proves nothing either way.

## The global shortcut does not fire

Something else may have taken it. System Settings, Shortcuts, Kavverna shows what is
registered, and conflicts are resolved there like any other shortcut.

## Mute all leaves one microphone live

Some devices, USB headsets in particular, report the mute as applied while `pactl` still shows
the input unmuted. A node's mute and a device route's mute are different layers and each tool
reads a different one. Until that is settled, mute all is not claimed to cover every input:
check with `pactl list sources short` if it matters.
