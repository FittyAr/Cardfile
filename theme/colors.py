import flet as ft

class ThemeColors:
    # Definición de temas
    THEMES = {
        "blue": {
            "primary": ft.Colors.BLUE_400,
            "primary_light": ft.Colors.BLUE_50,
            "primary_dark": ft.Colors.BLUE_700,
            "accent": ft.Colors.BLUE_ACCENT_400,
        },
        "emerald": {
            "primary": ft.Colors.GREEN_400,
            "primary_light": ft.Colors.GREEN_50,
            "primary_dark": ft.Colors.GREEN_700,
            "accent": ft.Colors.GREEN_ACCENT_400,
        },
        "sunset": {
            "primary": ft.Colors.ORANGE_400,
            "primary_light": ft.Colors.ORANGE_50,
            "primary_dark": ft.Colors.ORANGE_700,
            "accent": ft.Colors.ORANGE_ACCENT_400,
        },
        "midnight": {
            "primary": ft.Colors.INDIGO_400,
            "primary_light": ft.Colors.INDIGO_50,
            "primary_dark": ft.Colors.INDIGO_700,
            "accent": ft.Colors.INDIGO_ACCENT_400,
        },
        "rose": {
            "primary": ft.Colors.PINK_400,
            "primary_light": ft.Colors.PINK_50,
            "primary_dark": ft.Colors.PINK_700,
            "accent": ft.Colors.PINK_ACCENT_400,
        }
    }

    @staticmethod
    def get_colors(theme_name="blue"):
        return ThemeColors.THEMES.get(theme_name, ThemeColors.THEMES["blue"])
