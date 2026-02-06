import flet as ft
from config.config import Config
from config.runtime import is_web_runtime
from theme.manager import ThemeManager

theme_manager = ThemeManager()

def create_navigation_bar(page: ft.Page, on_change_handler):
    """Crea y retorna la barra de navegación"""
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['navigation']
    is_web = is_web_runtime(page)
    
    destinations = [
        ft.NavigationBarDestination(
            icon=ft.Icons.ADD,
            selected_icon=ft.Icons.ADD_CIRCLE,
            label=t['new'],
        ),
        ft.NavigationBarDestination(
            icon=ft.Icons.EDIT,
            selected_icon=ft.Icons.EDIT_ROUNDED,
            label=t['edit'],
        ),
        ft.NavigationBarDestination(
            icon=ft.Icons.DELETE,
            selected_icon=ft.Icons.DELETE_FOREVER,
            label=t['delete'],
        ),
        ft.NavigationBarDestination(
            icon=ft.Icons.RECYCLING,
            selected_icon=ft.Icons.RECYCLING_ROUNDED,
            label=t['recycle'],
        ),
        ft.NavigationBarDestination(
            icon=ft.Icons.SETTINGS_OUTLINED,
            selected_icon=ft.Icons.SETTINGS,
            label=t['settings'],
        ),
    ]
    if not is_web:
        destinations.append(
            ft.NavigationBarDestination(
                icon=ft.Icons.EXIT_TO_APP,
                selected_icon=ft.Icons.EXIT_TO_APP_ROUNDED,
                label=t['exit'],
            )
        )

    return ft.NavigationBar(
        selected_index=0,
        destinations=destinations,
        on_change=on_change_handler,
        bgcolor=theme_manager.navbar_bg,
        height=theme_manager.navbar_height,
    ) 
