#!/usr/bin/python3
"""Drive a live IBus engine and reconstruct what the user would see.

Usage: drive.py <dest-suffix> <engine-name> <battery-file> [--json out.json]

Each battery line is a sequence of space-separated tokens (see battery.txt).
A fresh engine object is created for every case, and a trailing SPACE is
appended so the word is committed; the trailing space is stripped from the
reported text.
"""
import sys, json, time, gi
gi.require_version('IBus', '1.0')
from gi.repository import IBus, GLib, Gio

dest_suffix, engine_name, battery_file = sys.argv[1], sys.argv[2], sys.argv[3]
out_json = None
if '--json' in sys.argv:
    out_json = sys.argv[sys.argv.index('--json') + 1]

bus = IBus.Bus()
conn = bus.get_connection()
dest = 'org.freedesktop.IBus.' + dest_suffix

KEYCODE = {
    'a': 38, 'b': 56, 'c': 54, 'd': 40, 'e': 26, 'f': 41, 'g': 42, 'h': 43,
    'i': 31, 'j': 44, 'k': 45, 'l': 46, 'm': 58, 'n': 57, 'o': 32, 'p': 33,
    'q': 24, 'r': 27, 's': 39, 't': 28, 'u': 30, 'v': 55, 'w': 25, 'x': 53,
    'y': 29, 'z': 52, ' ': 65, '1': 10, '2': 11, '3': 12, '4': 13, '5': 14,
    '6': 15, '7': 16, '8': 17, '9': 18, '0': 19, '[': 34, ']': 35, '\\': 51,
    ';': 47, "'": 48, ',': 59, '.': 60, '/': 61, '-': 20, '=': 21, '`': 49,
}
SPECIAL = {'ESC': (0xff1b, 9), 'BACK': (0xff08, 22), 'RET': (0xff0d, 36),
           'DEL': (0xffff, 119), 'TAB': (0xff09, 23), 'HOME': (0xff50, 110),
           'LEFT': (0xff51, 113), 'RIGHT': (0xff53, 114), 'UP': (0xff52, 111),
           'DOWN': (0xff54, 116), 'END': (0xff57, 115), 'PGUP': (0xff55, 112),
           'PGDN': (0xff56, 117), 'SPACE': (0x20, 65)}

state = {'events': [], 'committed': [], 'preedit': ''}


def on_signal(conn_, sender, path, iface, sig, params):
    if iface != 'org.freedesktop.IBus.Engine':
        return
    up = params.unpack()
    state['events'].append([sig, up])
    if sig == 'UpdatePreeditText':
        state['preedit'] = up[0][2]
    elif sig == 'CommitText':
        state['committed'].append(up[0][2])
        state['preedit'] = ''


conn.signal_subscribe(dest, 'org.freedesktop.IBus.Engine', None, None, None,
                      Gio.DBusSignalFlags.NONE, on_signal)


def pump():
    ctx = GLib.MainContext.default()
    for _ in range(8):
        while ctx.pending():
            ctx.iteration(False)
        time.sleep(0.001)


def call(path, iface, method, args=None, timeout=5000):
    if args is None:
        args = GLib.Variant('()', ())
    try:
        r = conn.call_sync(dest, path, iface, method, args, None,
                           Gio.DBusCallFlags.NONE, timeout, None)
        pump()
        return r.unpack()
    except GLib.Error as e:
        return ('ERROR', e.message)


def new_engine():
    res = call('/org/freedesktop/IBus/Factory', 'org.freedesktop.IBus.Factory',
               'CreateEngine', GLib.Variant('(s)', (engine_name,)))
    if not res or not isinstance(res, tuple):
        sys.exit('CreateEngine failed: %r' % (res,))
    path = res[0]
    call(path, 'org.freedesktop.IBus.Engine', 'Enable')
    call(path, 'org.freedesktop.IBus.Engine', 'FocusIn')
    caps = int(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS |
               IBus.Capabilite.SURROUNDING_TEXT)
    call(path, 'org.freedesktop.IBus.Engine', 'SetCapabilities', GLib.Variant('(u)', (caps,)))
    call(path, 'org.freedesktop.IBus.Engine', 'SetCursorLocation', GLib.Variant('(iiii)', (10, 20, 0, 0)))
    return path


def send(path, tok):
    mods = 0
    base = tok
    if base.upper() in SPECIAL:
        kv, kc = SPECIAL[base.upper()]
    elif len(base) == 2 and base[0] in 'SCAN' and base[1] in KEYCODE:
        mods |= {'S': 1, 'C': 4, 'A': 8, 'N': 16}[base[0]]
        kv = ord(base[1])
        kc = KEYCODE[base[1]]
    elif len(base) == 1 and base in KEYCODE:
        kv = ord(base)
        kc = KEYCODE[base]
    elif len(base) == 1:
        kv = ord(base)
        kc = 0
    else:
        raise SystemExit('bad token %r' % tok)
    r = call(path, 'org.freedesktop.IBus.Engine', 'ProcessKeyEvent',
             GLib.Variant('(uuu)', (kv, kc, mods)))
    return r[0] if isinstance(r, tuple) else None


results = []
for line in open(battery_file, encoding='utf-8'):
    line = line.rstrip('\n')
    if not line or line.startswith('#'):
        continue
    path = new_engine()
    state['events'] = []
    state['committed'] = []
    state['preedit'] = ''
    handled = []
    cmd_log = []
    for tok in line.split(' '):
        if tok.startswith('@'):
            cmd = tok[1:]
            m = {'focusout': 'FocusOut', 'focusin': 'FocusIn', 'reset': 'Reset',
                 'disable': 'Disable', 'enable': 'Enable'}[cmd]
            call(path, 'org.freedesktop.IBus.Engine', m)
            cmd_log.append([cmd, ''.join(state['committed']), state['preedit']])
            continue
        h = send(path, tok)
        handled.append(h)
        # simulate the client inserting the key when the engine declines it
        ins = None
        if h is False:
            if len(tok) == 1 and tok in KEYCODE:
                ins = tok
            elif len(tok) == 2 and tok[0] == 'S' and tok[1] in KEYCODE:
                ins = tok[1].upper()
        if ins is not None:
            state['committed'].append(ins)
            state['preedit'] = ''
    space_handled = send(path, 'SPACE')
    if space_handled is False:
        # the client inserts the space the engine declined
        state['committed'].append(' ')
    text = ''.join(state['committed']) + state['preedit']
    # strip the single trailing space that terminated the word
    stripped = text[:-1] if text.endswith(' ') else text
    results.append({'keys': line, 'handled': handled, 'text': text,
                    'space_handled': space_handled,
                    'word': stripped,
                    'double_space': stripped.endswith(' '),
                    'committed': state['committed'], 'preedit': state['preedit'],
                    'cmds': cmd_log, 'events': state['events']})

if out_json:
    with open(out_json, 'w', encoding='utf-8') as f:
        json.dump(results, f, ensure_ascii=False, indent=1)
    print('wrote', out_json, len(results), 'cases')

for r in results:
    flag = ' !!DOUBLE' if r['double_space'] else ''
    print('%-34s -> %-22s | handled=%s | commits=%s%s' % (
        r['keys'], repr(r['word']),
        ''.join('T' if h else ('F' if h is not None else '?') for h in r['handled']),
        r['committed'], flag))
