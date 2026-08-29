# Privacy

Kavverna has no network code. Not a disabled telemetry switch, not an update check that can be
turned off: no HTTP client is linked into the binary at all. You can check with
`cargo tree | grep -iE 'reqwest|hyper|curl'` and get nothing back.

## What is stored, and where

| What | Where | Who can read it |
|---|---|---|
| Clipboard entries | `$XDG_DATA_HOME/kavverna/clipboard.db` | your user only, `0600` |
| Copied images | `$XDG_DATA_HOME/kavverna/clipboard-images/` | your user only, `0700` |
| Settings | `$XDG_CONFIG_HOME/kavverna/settings.json` | your user only, `0600` |

On a normal machine `$XDG_DATA_HOME` is `~/.local/share` and `$XDG_CONFIG_HOME` is `~/.config`.

## What is never stored

- **Anything an application marks as a secret.** A password manager offers the mime type
  `x-kde-passwordManagerHint` beside the secret. Kavverna sees that on the offer and never
  reads the content, so it does not reach the process, let alone the disk.
- **Text shaped like a secret,** while the setting for it is on. Five shapes: a string with no
  spaces mixing letters, digits and symbols; one containing a word like password or token; a
  JSON web token, recognised by its three dotted parts rather than by length, since those run
  past what the general rule covers; a key block, recognised by its `-----BEGIN` line, since it
  is mostly line breaks and the general rule rejects anything with one; and a URL of any scheme
  carrying a user name or password, which is what a connection string is.
- **Anything at all, with the history switched off.** The compositor still reports that a copy
  happened, which is what the clear timer needs, but the content is not taken. There is a test
  that copies something with reading off and fails if it arrives.

## Logs

Nothing you copy is written to the log. Sizes, counts and mime types are, at debug level, so a
problem can be diagnosed without the content. Logging goes to standard error and nowhere else.

## Deleting it

The panel clears everything unpinned in one click, and an entry one at a time. To remove the
lot, including what is pinned, delete `$XDG_DATA_HOME/kavverna`. Nothing is kept anywhere else.
