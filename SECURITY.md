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

## What it deliberately does not do

- Synthetic input. `ydotool` would need membership of the `input` group, which makes every
  input device readable by any process running as you. Nothing today requires it, and anything
  that does will say so before it is switched on.
- Fan control. It writes PWM as root and a fan left stopped can damage hardware. It stays out
  until it can have its own privileged daemon with a heartbeat and a thermal watchdog.
