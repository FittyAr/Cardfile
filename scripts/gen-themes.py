#!/usr/bin/env python3
"""Generate assets/themes/cardfile.json (gpui-component theme format).

Each theme is described by a small base palette; every component color is derived
from it so all themes stay consistent. Run: python scripts/gen-themes.py
"""
import json
import pathlib

# name, mode, primary, primary_fg, accent, bg, sidebar, surface, text, subtext
THEMES = [
    # Ported from Cardfile 1.x
    ("Snow", "light", "#2D5AFE", "#FFFFFF", "#00C2FF", "#FFFFFF", "#F8F9FD", "#FFFFFF", "#1A1C1E", "#6C757D"),
    ("Midnight", "dark", "#3B82F6", "#FFFFFF", "#60A5FA", "#0F172A", "#020617", "#1E293B", "#F8FAFC", "#94A3B8"),
    ("Cyber", "dark", "#FF007A", "#FFFFFF", "#00FFF0", "#0D0221", "#05010D", "#1A0B2E", "#FFFFFF", "#C77DFF"),
    ("Forest", "light", "#059669", "#FFFFFF", "#34D399", "#F0FDF4", "#DCFCE7", "#FFFFFF", "#064E3B", "#047857"),
    ("Nordic", "light", "#4C566A", "#ECEFF4", "#88C0D0", "#E5E9F0", "#D8DEE9", "#ECEFF4", "#2E3440", "#4C566A"),
    ("Sunset", "light", "#FF6B3D", "#FFFFFF", "#FFB347", "#FFF5EE", "#FFE4D6", "#FFFFFF", "#4A2C2A", "#B55D4A"),
    ("Lavender", "light", "#7C3AED", "#FFFFFF", "#A78BFA", "#FAF5FF", "#F3E8FF", "#FFFFFF", "#3B0764", "#6D28D9"),
    ("Ocean", "dark", "#2E8BC0", "#FFFFFF", "#B1D4E0", "#001C2E", "#00111D", "#0A2A43", "#E8F1F8", "#8FB5CC"),
    ("Graphite", "dark", "#9CA3AF", "#111827", "#FBBF24", "#111827", "#0B1220", "#1F2937", "#F9FAFB", "#9CA3AF"),
    # New in Cardfile 2
    ("Paper", "light", "#8B5E34", "#FFFFFF", "#D4A373", "#FBF8F1", "#F3EDE0", "#FFFDF8", "#3D3426", "#7A6A55"),
    ("Solarized Light", "light", "#268BD2", "#FDF6E3", "#2AA198", "#FDF6E3", "#EEE8D5", "#FDF6E3", "#586E75", "#93A1A1"),
    ("Solarized Dark", "dark", "#268BD2", "#FDF6E3", "#2AA198", "#002B36", "#00212B", "#073642", "#EEE8D5", "#93A1A1"),
    ("Rose", "dark", "#EB6F92", "#191724", "#F6C177", "#191724", "#13111C", "#1F1D2E", "#E0DEF4", "#908CAA"),
    ("High Contrast", "dark", "#FFD400", "#000000", "#00E5FF", "#000000", "#000000", "#0A0A0A", "#FFFFFF", "#D0D0D0"),
]


def hex2rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) for i in (0, 2, 4))


def rgb2hex(c):
    return "#" + "".join(f"{max(0, min(255, round(v))):02x}" for v in c)


def mix(a, b, t):
    """Blend color a toward b by t (0..1)."""
    ca, cb = hex2rgb(a), hex2rgb(b)
    return rgb2hex(tuple(x + (y - x) * t for x, y in zip(ca, cb)))


def alpha(c, a):
    return c + f"{round(a * 255):02x}"


def theme(name, mode, primary, primary_fg, accent, bg, sidebar, surface, text, subtext):
    dark = mode == "dark"
    border = mix(bg, text, 0.16 if dark else 0.12)
    muted = mix(bg, text, 0.08 if dark else 0.05)
    hover = mix(bg, text, 0.06 if dark else 0.04)
    danger = "#F87171" if dark else "#DC2626"
    success = "#4ADE80" if dark else "#16A34A"
    warning = "#FBBF24" if dark else "#D97706"
    info = "#38BDF8" if dark else "#0284C7"
    colors = {
        "background": bg,
        "foreground": text,
        "border": border,
        "input.border": border,
        "ring": primary,
        "caret": primary,
        "selection.background": alpha(primary, 0.35),
        "overlay": alpha("#000000", 0.45 if dark else 0.12),
        "window.border": border,
        "primary.background": primary,
        "primary.hover.background": mix(primary, text if dark else bg, 0.15),
        "primary.active.background": mix(primary, bg, 0.2),
        "primary.foreground": primary_fg,
        "secondary.background": muted,
        "secondary.hover.background": hover,
        "secondary.active.background": border,
        "secondary.foreground": text,
        "muted.background": muted,
        "muted.foreground": subtext,
        "accent.background": hover,
        "accent.foreground": text,
        "popover.background": surface,
        "popover.foreground": text,
        "link.foreground": accent if dark else primary,
        "link.hover.foreground": mix(accent if dark else primary, text, 0.25),
        "link.active.foreground": accent if dark else primary,
        "list.background": bg,
        "list.hover.background": hover,
        "list.active.background": alpha(primary, 0.18),
        "list.active.border": primary,
        "list.even.background": bg,
        "list.head.background": muted,
        "sidebar.background": sidebar,
        "sidebar.foreground": text,
        "sidebar.border": border,
        "sidebar.accent.background": alpha(primary, 0.18),
        "sidebar.accent.foreground": text,
        "sidebar.primary.background": primary,
        "sidebar.primary.foreground": primary_fg,
        "tab.background": alpha(bg, 0),
        "tab.active.background": bg,
        "tab.active.foreground": text,
        "tab.foreground": subtext,
        "tab_bar.background": muted,
        "tab_bar.segmented.background": muted,
        "table.background": bg,
        "table.head.background": muted,
        "table.head.foreground": subtext,
        "table.hover.background": hover,
        "table.active.background": alpha(primary, 0.18),
        "table.active.border": primary,
        "table.even.background": bg,
        "table.row.border": border,
        "title_bar.background": sidebar,
        "title_bar.border": border,
        "status_bar.background": sidebar,
        "status_bar.border": border,
        "scrollbar.background": alpha(bg, 0),
        "scrollbar.thumb.background": alpha(subtext, 0.5),
        "scrollbar.thumb.hover.background": subtext,
        "switch.background": border,
        "slider.bar.background": primary,
        "slider.thumb.background": primary_fg,
        "progress_bar.background": primary,
        "skeleton.background": muted,
        "drag_border": primary,
        "drop_target.background": alpha(primary, 0.2),
        "danger.background": danger,
        "danger.foreground": "#FFFFFF",
        "success.background": success,
        "success.foreground": "#FFFFFF",
        "warning.background": warning,
        "warning.foreground": "#FFFFFF",
        "info.background": info,
        "info.foreground": "#FFFFFF",
        "chart.1": primary,
        "chart.2": accent,
        "chart.3": success,
        "chart.4": warning,
        "chart.5": danger,
    }
    return {"name": name, "mode": mode, "colors": colors}


def main():
    out = {
        "name": "Cardfile",
        "author": "FittyAr",
        "themes": [theme(*t) for t in THEMES],
    }
    path = pathlib.Path(__file__).resolve().parent.parent / "assets" / "themes" / "cardfile.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(THEMES)} themes to {path}")


if __name__ == "__main__":
    main()
