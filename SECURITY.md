# Security

## Reporting

Open a [private security advisory](https://github.com/novasvilla/kavverna/security/advisories/new)
rather than a public issue. Expect a first reply within a few days.

## What this application can reach

Worth knowing before you decide to trust it:

- **Everything you copy**, while the clipboard history is on, through
  `ext_data_control_manager_v1`. That is the same access Klipper has.
- **The session bus**, where it registers a global shortcut and serves
  `dev.kavverna.Shell`. Any process on your session can call that interface, which can open the
  panel, hold off sleep and change the default audio output. It cannot read the history.
- **The system bus**, read only, for the signal logind sends before suspending.
- **`/proc` and `/sys`**, read only, for the system monitor.

It does not run as root, does not install a daemon and does not ask for any elevated
permission. There is no network code.

## Synthetic input

The pointer nudge moves the pointer through `ydotool`, and pressing a key uses the same route.
That needs `ydotool` installed and its daemon running, which in practice means membership of
the `input` group, and that makes every input device readable by any process running as you.

Nothing else uses it. The feature is off by default and does nothing at all without `ydotool`,
so declining to install it declines the whole cost. Paste as plain text and pasting from the
picker will need the same thing when they land, and will say so before they are switched on.

## What it deliberately does not do
- Fan control. It writes PWM as root and a fan left stopped can damage hardware. It stays out
  until it can have its own privileged daemon with a heartbeat and a thermal watchdog.
