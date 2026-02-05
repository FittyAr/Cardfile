import flet as ft

class ThemeColors:
    # Definición de temas
    THEMES = {
        "snow": {
            "name": "Snow White",
            "is_dark": False,
            "primary": "#2D5AFE",
            "primary_light": "#EBF0FF",
            "primary_dark": "#1A3BB0",
            "accent": "#00C2FF",
            "bg": "#FFFFFF",
            "sidebar_bg": "#F8F9FD",
            "card_bg": "#FFFFFF",
            "text": "#1A1C1E",
            "subtext": "#6C757D"
        },
        "midnight": {
            "name": "Midnight Noir",
            "is_dark": True,
            "primary": "#3B82F6",
            "primary_light": "#1E293B",
            "primary_dark": "#1D4ED8",
            "accent": "#60A5FA",
            "bg": "#0F172A",
            "sidebar_bg": "#020617",
            "card_bg": "#1E293B",
            "text": "#F8FAFC",
            "subtext": "#94A3B8"
        },
        "cyber": {
            "name": "Cyber Neon",
            "is_dark": True,
            "primary": "#FF007A",
            "primary_light": "#2D001A",
            "primary_dark": "#B30056",
            "accent": "#00FFF0",
            "bg": "#0D0221",
            "sidebar_bg": "#05010D",
            "card_bg": "#1A0B2E",
            "text": "#FFFFFF",
            "subtext": "#FF007A"
        },
        "forest": {
            "name": "Eco Forest",
            "is_dark": False,
            "primary": "#059669",
            "primary_light": "#ECFDF5",
            "primary_dark": "#047857",
            "accent": "#34D399",
            "bg": "#F0FDF4",
            "sidebar_bg": "#DCFCE7",
            "card_bg": "#FFFFFF",
            "text": "#064E3B",
            "subtext": "#059669"
        },
        "nordic": {
            "name": "Nordic Frost",
            "is_dark": False,
            "primary": "#4C566A",
            "primary_light": "#ECEFF4",
            "primary_dark": "#2E3440",
            "accent": "#88C0D0",
            "bg": "#E5E9F0",
            "sidebar_bg": "#D8DEE9",
            "card_bg": "#ECEFF4",
            "text": "#2E3440",
            "subtext": "#4C566A"
        }
    }

    @staticmethod
    def get_colors(theme_name="snow"):
        return ThemeColors.THEMES.get(theme_name, ThemeColors.THEMES["snow"])
