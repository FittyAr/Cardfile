import flet as ft
from config.config import Config

def create_navigation_bar(page: ft.Page, on_change_handler):
    """Crea y retorna la barra de navegación"""
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['navigation']
    
    return ft.NavigationBar(
        selected_index=0,
        destinations=[
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
                icon=ft.Icons.EXIT_TO_APP,
                selected_icon=ft.Icons.EXIT_TO_APP_ROUNDED,
                label=t['exit'],
            ),
        ],
        on_change=on_change_handler,
        bgcolor=ft.Colors.SURFACE,
        height=65,
    ) 