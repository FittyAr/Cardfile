"""Drive the Cardfile window for screenshots: start / shot / click / key / move / close."""

import ctypes
import ctypes.wintypes as wt
import os
import subprocess
import sys
import time

from PIL import ImageGrab

u = ctypes.windll.user32
u.SetProcessDPIAware()
EXE = os.environ.get("CARDFILE_EXE", r"target\release\cardfile.exe")


def find():
    found = []
    pids = set()
    out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq cardfile.exe", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
    for line in out.splitlines():
        parts = line.split('","')
        if len(parts) > 1:
            pids.add(int(parts[1]))

    @ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    def cb(h, _):
        pid = wt.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(pid))
        if pid.value in pids and u.IsWindowVisible(h):
            r = rect(h)
            if r[2] - r[0] > 300:
                found.append(h)
        return True

    u.EnumWindows(cb, 0)
    return found if ALL else (found[-1] if found else None)


ALL = False


def rect(h):
    r = wt.RECT()
    ctypes.windll.dwmapi.DwmGetWindowAttribute(h, 9, ctypes.byref(r), ctypes.sizeof(r))
    return r.left, r.top, r.right, r.bottom


def focus(h):
    u.ShowWindow(h, 9)
    u.SetForegroundWindow(h)
    time.sleep(0.3)


def close():
    subprocess.run(["taskkill", "/F", "/IM", "cardfile.exe"], capture_output=True)


VK = {"ctrl": 0x11, "shift": 0x10, "alt": 0x12, "enter": 0x0D, "esc": 0x1B, "tab": 0x09, "down": 0x28, "up": 0x26,
      "left": 0x25, "right": 0x27, "space": 0x20, "back": 0x08, "f1": 0x70, "comma": 0xBC, "home": 0x24, "end": 0x23}


def vk(name):
    if name in VK:
        return VK[name]
    if len(name) == 1:
        return ord(name.upper())
    raise SystemExit(f"unknown key {name}")


def key(combo):
    keys = [vk(k) for k in combo.lower().split("+")]
    for k in keys:
        u.keybd_event(k, 0, 0, 0)
        time.sleep(0.03)
    for k in reversed(keys):
        u.keybd_event(k, 0, 2, 0)
        time.sleep(0.03)


def type_text(text):
    INPUT_KEYBOARD, KEYEVENTF_UNICODE, KEYUP = 1, 4, 2

    class KI(ctypes.Structure):
        _fields_ = [("wVk", wt.WORD), ("wScan", wt.WORD), ("dwFlags", wt.DWORD), ("time", wt.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class INP(ctypes.Structure):
        _fields_ = [("type", wt.DWORD), ("ki", KI), ("pad", ctypes.c_ubyte * 8)]

    for ch in text:
        for flags in (KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYUP):
            i = INP(INPUT_KEYBOARD, KI(0, ord(ch), flags, 0, 0))
            u.SendInput(1, ctypes.byref(i), ctypes.sizeof(INP))
        time.sleep(0.02)


def main():
    cmd, *a = sys.argv[1:]
    if cmd == "start":
        close()
        time.sleep(0.6)
        subprocess.Popen([EXE, "--data-dir", a[0]])
        h = None
        for _ in range(80):
            h = find()
            if h:
                break
            time.sleep(0.25)
        w, hh = (int(a[1]), int(a[2])) if len(a) > 2 else (1600, 1000)
        u.ShowWindow(h, 1)
        u.MoveWindow(h, 40, 40, w, hh, True)
        focus(h)
        time.sleep(2.5)
        print("dpi", u.GetDpiForWindow(h), rect(h))
        return
    if cmd == "list":
        global ALL
        ALL = True
        for h in find():
            buf = ctypes.create_unicode_buffer(256)
            u.GetWindowTextW(h, buf, 256)
            print(h, buf.value, rect(h))
        return
    if cmd == "shotwin":
        ALL = True
        h = int(a[1])
        focus(h)
        time.sleep(0.5)
        ImageGrab.grab(bbox=rect(h), all_screens=True).save(a[0])
        print("saved", a[0], rect(h))
        return
    if cmd == "close":
        close()
        return
    h = find()
    focus(h)
    r = rect(h)
    if cmd == "shot":
        time.sleep(0.5)
        ImageGrab.grab(bbox=r, all_screens=True).save(a[0])
        print("saved", a[0], r)
    elif cmd == "click":
        for spec in a:
            right = spec.endswith("r")
            dbl = spec.endswith("d")
            x, y = (int(v) for v in spec.rstrip("rd").split(","))
            u.SetCursorPos(r[0] + x, r[1] + y)
            time.sleep(0.1)
            for _ in range(2 if dbl else 1):
                down, up = (8, 16) if right else (2, 4)
                u.mouse_event(down, 0, 0, 0, 0)
                u.mouse_event(up, 0, 0, 0, 0)
                time.sleep(0.05)
            time.sleep(0.6)
    elif cmd == "move":
        x, y = (int(v) for v in a[0].split(","))
        u.SetCursorPos(r[0] + x, r[1] + y)
        time.sleep(0.6)
    elif cmd == "key":
        for combo in a:
            key(combo)
            time.sleep(0.4)
    elif cmd == "type":
        type_text(a[0])
        time.sleep(0.6)


main()
