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
                icon=ft.icons.ADD,
                selected_icon=ft.icons.ADD_CIRCLE,
                label=t['new'],
            ),
            ft.NavigationBarDestination(
                icon=ft.icons.EDIT,
                selected_icon=ft.icons.EDIT_ROUNDED,
                label=t['edit'],
            ),
            ft.NavigationBarDestination(
                icon=ft.icons.DELETE,
                selected_icon=ft.icons.DELETE_FOREVER,
                label=t['delete'],
            ),
            ft.NavigationBarDestination(
                icon=ft.icons.RECYCLING,
                selected_icon=ft.icons.RECYCLING_ROUNDED,
                label=t['recycle'],
            ),
            ft.NavigationBarDestination(
                icon=ft.icons.EXIT_TO_APP,
                selected_icon=ft.icons.EXIT_TO_APP_ROUNDED,
                label=t['exit'],
            ),
        ],
        on_change=on_change_handler,
        bgcolor=ft.colors.SURFACE_VARIANT,
        height=65,
    ) 