# Privacy

Kavverna has no network code. Not a disabled telemetry switch, not an update check that can be
turned off: no HTTP client is linked into the binary at all. You can check with
`cargo tree | grep -iE 'reqwest|hyper|curl'` and get nothing back.

## The AI agents utility and Claude Code

While the panel is open, the AI agents utility can ask the Claude Code installed for your user
how much of its allowance is used, at most once a minute and less often after a failure. It
starts `claude` with no tools, no MCP servers, no hooks and no saved session, sends it the same
request `/usage` makes, and stops it as soon as it answers. Claude Code then contacts Anthropic
with its own sign-in, exactly as when you type `/usage`. No prompt is sent, so no model runs.

Kavverna makes no network request for this and reads no credential. It removes Claude session
markers and `ANTHROPIC_` variables from the environment it passes, so the sign-in used is the
one Claude Code keeps, and carries over the telemetry opt-outs and proxy settings you keep in
Claude Code's `settings.json`. From the answer it keeps only the percentages, reset times, the
model a cap applies to and that cap's share of the weekly limit, in memory. Account, plan and
spending details in the same answer are discarded and never logged.

When the AI agents utility is not installed, Kavverna never starts Claude Code. When Claude Code
cannot be asked, its allowance is read from files on this machine only: Kavverna's own status
line record, or the copy the `claude-statusbar` tool leaves in
`$XDG_CACHE_HOME/claude-statusbar/last_stdin.json`, of which only the rate limit windows are
read.

## What is stored, and where

| What | Where | Who can read it |
|---|---|---|
| Clipboard entries | `$XDG_DATA_HOME/kavverna/clipboard.db` | your user only, `0600` |
| Copied images | `$XDG_DATA_HOME/kavverna/clipboard-images/` | your user only, `0700` |
| The shelf | `$XDG_DATA_HOME/kavverna/shelf/` | your user only, `0700` |
| Settings | `$XDG_CONFIG_HOME/kavverna/settings.json` | your user only, `0600` |

The shelf stores local files by path, never by copy. Only content with no file behind it, an
image dragged out of a browser or a snippet of dropped text, is written there, and taking an
item off the shelf deletes what the shelf itself wrote. A dropped web address is kept as text
and never fetched.

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
- **Anything at all, with both history and link cleaning switched off.** The compositor still
  reports that a copy happened for the clear timer, but the content is not taken. Link cleaning
  must read copied text to decide whether a URL needs rewriting. A compositor test covers the
  no-reading case.

## Logs

Nothing you copy is written to the log. Sizes, counts and mime types are, at debug level, so a
problem can be diagnosed without the content. Logging goes to standard error and nowhere else.

## Deleting it

The panel clears everything unpinned in one click, and an entry one at a time. To remove the
lot, including what is pinned, delete `$XDG_DATA_HOME/kavverna`. Nothing is kept anywhere else.
