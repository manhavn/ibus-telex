# What this engine does differently on Wayland

Vietnamese IMEs have a bad reputation under Wayland: characters go missing,
words get inserted twice, a pending word follows you into the next window,
and shortcuts stop working.  Most of that comes from the input method
protocol being used differently: on Wayland the pre-edit is not something the
application renders, focus changes are much more frequent, and there is no
global input focus or cursor position to query.

Every item below is a decision in this code base, not a configuration
option.

## 1. No X11 in the loop

The engine never opens an X display, never calls `XGetInputFocus`, never uses
XIM and never needs a keycode-to-keysym table.  `src/ibus/keys.rs` only knows
key values and modifier masks, and the panel gets its position from
`SetCursorLocation`, which the input context sends.

## 2. Keys are never forwarded

`ForwardKeyEvent` is not used at all.  A key the engine does not want is
declined by returning `false` from `ProcessKeyEvent` (see
`Engine::process_key_event`), which makes the daemon hand the original event
to the application.  Forwarded events are the usual reason a character
appears twice - or not at all - under a Wayland client.

## 3. Surrounding text is ignored

`SetSurroundingText` is accepted and thrown away, and the engine never emits
`DeleteSurroundingText`.  Wayland clients (and every toolkit on top of
`text-input-v3`) report surrounding text that can be stale or reordered, and
an engine that edits the buffer based on it corrupts the text.  The cost is
that a word is committed rather than fixed in place, which is the safer
trade.

## 4. The word is never dropped

`FocusOut` and `Disable` commit the pending word instead of discarding it
(`Engine::focus_out`, `Engine::disable`); only `Reset` - which the client
sends when *it* wants a clean slate - throws it away.  Unikey drops the word
on both, which is why a half-typed word disappears when you click a
notification, and keep in mind that under Wayland focus is lost at the
slightest provocation.

`tests/protocol.rs` asserts this: typing `dd`, then `FocusOut`, must commit
`đ`.

## 5. No double commits

The pre-edit is always published with focus mode `CLEAR`
(`keys::PREEDIT_CLEAR` in `Engine::emit_preedit`) *and* the engine commits by
itself.  With mode `COMMIT` the framework may commit the pre-edit on focus
change as well, which is how one word ends up inserted twice.

## 6. No stale pre-edit

Committing emits `CommitText` and then an empty, invisible pre-edit
(`Engine::emit_commit`), and an empty pre-edit is emitted on `Enable`,
`Reset` and after the buffer is emptied with Backspace.  A pre-edit left on
screen while the application has moved on is what makes the next keystroke
land in the wrong place.

## 7. Keys that move the cursor commit first

Return, Tab, Escape, the arrows, Home/End, PageUp/PageDown, Insert and Delete
(`keys::is_navigation`) commit the pending word and then let the application
see the key.  Otherwise the application moves the cursor while the engine
still overlays a pre-edit at the old position.

## 8. Application shortcuts are left alone, and only those

Any event carrying Control, Alt, Super, Hyper or Meta is passed straight
through (`keys::is_shortcut`).  ibus-unikey swallows those: with it, Ctrl+A
inserts the letter `a` into the word.  `tests/protocol.rs` checks Ctrl+C,
Ctrl+A and Ctrl+V.

The other direction matters just as much: *only* those modifiers count.  Num
Lock is `MOD2` and is set on **every** keystroke while it is on, AltGr is
`MOD5`, Scroll Lock shows up as `MOD3`.  An engine that treats anything it
does not recognise as a shortcut is dead on a perfectly ordinary keyboard -
that is not hypothetical, it is exactly the bug this engine shipped with:
with Num Lock on, every key was handed straight back to the application and
typing Vietnamese did nothing at all.  The reference engine, ibus-table,
looks at Control and Alt and nothing else.  `tests/keys.rs` pins the policy
and `tests/protocol.rs` types a whole word with `MOD2` set.

If it ever happens again, `IBUS_TELEX_DEBUG=1` makes the engine log every
key it is offered together with the state word and the decision:

```
ibus-telex: key 't' state=0x00000010 -> consumed
```

## 9. Password fields are left alone

`SetContentType` is honoured: with purpose `PASSWORD` or `PIN` the engine
stops handling keys and commits what it already had
(`State::is_secret`).  No pre-edit is ever shown next to a password.

## 10. No dead instance

The engine requests its bus name with `DO_NOT_QUEUE`
(`ibus::engine::run`): if a second copy was started - the daemon does that
when the same engine is used from two sessions - the new process exits
quietly instead of the two of them fighting over the name.

## 11. Installing without root

IBus only reads `/usr/share/ibus/component`, and it caches the component
directories it scanned in `~/.cache/ibus/bus/registry`, so a user component
directory is invisible until that cache is rebuilt.  `ibus-telex install`
writes a systemd user drop-in extending `IBUS_COMPONENT_PATH`, drops the
stale cache and restarts the daemon, which makes a rootless install work on
a GNOME Wayland session.  See the README for the details.

## Things that are not ours to fix

* Electron/Chromium applications only see input methods when started with
  `--enable-wayland-ime` (or `--ozone-platform-hint=auto`); without it the
  compositor never offers text input to the engine.
* `ibus-telex doctor` prints those reminders for the session it runs in.
