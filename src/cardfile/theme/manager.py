import flet as ft
from cardfile.theme.colors import ThemeColors
from cardfile.config.config import Config

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
        self._apply_colors(theme_name)

    def _apply_colors(self, theme_name):
        self.colors = ThemeColors.get_colors(theme_name)
        self.primary = self.colors["primary"]
        self.primary_light = self.colors["primary_light"]
        self.primary_dark = self.colors["primary_dark"]
        self.accent = self.colors["accent"]
        self.bg = self.colors["bg"]
        self.sidebar_bg = self.colors["sidebar_bg"]
        self.card_bg = self.colors["card_bg"]
        self.text = self.colors["text"]
        self.subtext = self.colors["subtext"]
        self.is_dark = self.colors["is_dark"]
        self.color_danger = ft.Colors.RED_400
        self.color_success = ft.Colors.GREEN_400
        self.space_1 = 1  # Separador fino en toolbar de markdown (components/markdown_editor.py)
        self.space_4 = 4  # Spacing compacto (tabs y listas, card_ui.py, Card.py)
        self.space_8 = 8  # Spacing básico entre controles y toolbars (markdown_editor.py, card_ui.py)
        self.space_12 = 12  # Spacing medio en modales y headers (NewCard.py, EditCard.py, Login.py, newUser.py, Recycle.py)
        self.space_16 = 16  # Padding de ítems de tarjeta/lista y gaps amplios (Card.py, Recycle.py)
        self.space_20 = 20  # Padding estándar de contenedores (card_ui.py, Card.py)
        self.text_size_sm = 12  # Etiquetas pequeñas y contador (card_ui.py, Card.py, Recycle.py)
        self.text_size_md = 14  # Texto base y campos (NewCard.py, EditCard.py, Login.py, newUser.py)
        self.text_size_lg = 18  # Título del sidebar (card_ui.py)
        self.text_size_xl = 20  # Marca “CardFile” (Login.py, newUser.py)
        self.text_size_xxl = 24  # Títulos de modales (NewCard.py, EditCard.py, Recycle.py)
        self.text_size_3xl = 28  # Títulos grandes de login/registro (Login.py, newUser.py)
        self.icon_size_sm = 16  # Iconos pequeños (card_ui.py)
        self.icon_size_md = 20  # Iconos de toolbars y acciones (markdown_editor.py, Recycle.py, MarkdownEditor.py)
        self.icon_size_lg = 28  # Iconos de títulos de modales (NewCard.py, EditCard.py, Recycle.py)
        self.icon_size_xl = 64  # Icono principal de login/registro (Login.py, newUser.py)
        self.input_width = 320  # Ancho estándar de campos de texto (NewCard.py, EditCard.py, Login.py, newUser.py)
        self.modal_width = 400  # Ancho de modales pequeños (NewCard.py, EditCard.py)
        self.modal_height = 320  # Alto fijo de modales pequeños (NewCard.py, EditCard.py)
        self.auth_card_width = 400  # Ancho de tarjetas de login/registro (Login.py, newUser.py)
        self.auth_card_padding = 40  # Padding de tarjetas de login/registro (Login.py, newUser.py)
        self.modal_padding = 30  # Padding interno de modales (NewCard.py, EditCard.py, Recycle.py)
        self.sidebar_width = 320  # Ancho del sidebar principal (card_ui.py)
        self.recycle_width = 600  # Ancho del modal de papelera (Recycle.py)
        self.recycle_height = 500  # Alto del modal de papelera (Recycle.py)
        self.navbar_height = 65  # Alto de barra de navegación (Navigation.py)
        self.navbar_bg = ft.Colors.SURFACE  # Color base de barra de navegación (Navigation.py)
        self.language_dropdown_width = 120  # Ancho del selector de idioma (Login.py)
        self.button_width = 130  # Botón estándar en modales (NewCard.py, EditCard.py, Recycle.py)
        self.button_height = 40  # Alto del botón estándar (NewCard.py, EditCard.py, Recycle.py)
        self.button_width_lg = 150  # Botón grande en auth (Login.py, newUser.py)
        self.button_height_lg = 45  # Alto del botón grande en auth (Login.py, newUser.py)
        self.button_radius = 8  # Radio del botón principal (card_ui.py, Login.py, NewCard.py, EditCard.py, Recycle.py, newUser.py)
        self.radius_sm = 8  # Radio de tabs y toolbars (card_ui.py, markdown_editor.py, MarkdownEditor.py)
        self.radius_md = 10  # Radio de ítems de lista (Card.py, Recycle.py)
        self.radius_lg = 20  # Radio de tarjetas/modales (Login.py, newUser.py, NewCard.py, EditCard.py, Recycle.py)
        self.radius_round = 50  # Radio circular del selector de tema (Card.py)
        self.divider_color = ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)
        self.border_color = ft.Colors.with_opacity(0.1, self.text)
        self.subtle_bg = ft.Colors.with_opacity(0.05, self.text)
        self.toolbar_divider_color = ft.Colors.with_opacity(0.2, self.text)
        self.selection_opacity = 0.3
        self.selection_bg = ft.Colors.with_opacity(0.1, self.text)
        self.modal_overlay_bg = ft.Colors.with_opacity(0.2, ft.Colors.BLACK)
        self.selected_text = ft.Colors.WHITE
        self.selected_subtext = ft.Colors.with_opacity(0.8, ft.Colors.WHITE)
        self.modal_overlay_blur = 10
        self.card_border = ft.Border.all(1, self.border_color)
        self.sidebar_border = ft.Border.only(right=ft.BorderSide(1, self.border_color))
        self.card_shadow = ft.BoxShadow(
            blur_radius=30,
            color=ft.Colors.with_opacity(0.1, ft.Colors.BLACK),
            offset=ft.Offset(0, 10),
        )
        self.primary_button_style = ft.ButtonStyle(
            shape=ft.RoundedRectangleBorder(radius=self.button_radius)
        )
        self.text_style_label = ft.TextStyle(color=self.subtext)
        self.tab_radius = ft.BorderRadius.only(top_left=self.radius_sm, top_right=self.radius_sm)

    def preview_theme(self, theme_name):
        self._apply_colors(theme_name)

    def set_theme(self, theme_name):
        self.config.set_theme(theme_name)
        self.refresh_colors()

    @property
    def current_theme_name(self):
        return self.config.current_theme
