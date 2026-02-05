import flet as ft
from theme.colors import ThemeColors
from config.config import Config

class ThemeManager:
    _instance = None
    
    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(ThemeManager, cls).__new__(cls)
            cls._instance.config = Config()
            cls._instance.refresh_colors()
        return cls._instance

    def refresh_colors(self):
        theme_name = self.config.current_theme
        self.colors = ThemeColors.get_colors(theme_name)
        self.primary = self.colors["primary"]
        self.primary_light = self.colors["primary_light"]
        self.primary_dark = self.colors["primary_dark"]
        self.accent = self.colors["accent"]

    def set_theme(self, theme_name):
        self.config.set_theme(theme_name)
        self.refresh_colors()

    @property
    def current_theme_name(self):
        return self.config.current_theme
