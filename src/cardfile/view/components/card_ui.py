"""
Componentes UI reutilizables para la vista de tarjetas

Este módulo proporciona funciones para crear componentes de interfaz:
- Sidebar con búsqueda y lista de tarjetas
- Header del panel principal
- Tabs personalizados para Editor/Preview
- Indicadores visuales (guardado, contador)

Uso:
    from View.components.card_ui import (
        create_sidebar,
        create_card_header,
        create_custom_tabs,
        create_save_indicator
    )
"""
import flet as ft
from typing import Callable, Tuple
from cardfile.theme.manager import ThemeManager

theme_manager = ThemeManager()


def create_search_field(on_change: Callable) -> ft.TextField:
    """
    Crea campo de búsqueda para filtrar tarjetas.
    
    Args:
        on_change: Callback llamado cuando cambia el texto de búsqueda
    
    Returns:
        ft.TextField configurado para búsqueda
    """
    return ft.TextField(
        hint_text="Buscar tarjetas...",
        prefix_icon=ft.Icons.SEARCH,
        focused_border_color=theme_manager.primary,
        border_color=theme_manager.border_color,
        on_change=on_change,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        hint_style=theme_manager.text_style_label,
    )


def create_sidebar(
    search_field: ft.TextField,
    cards_listview: ft.ListView,
    card_counter: ft.Text,
    new_card_callback: Callable,
    recycle_bin_callback: Callable,
    settings_callback: Callable,
    logout_callback: Callable
) -> ft.Container:
    """
    Crea el sidebar con búsqueda, lista de tarjetas y botones de acción.
    
    Args:
        search_field: Campo de búsqueda
        cards_listview: ListView con las tarjetas
        card_counter: Texto con el contador de tarjetas
        new_card_callback: Callback para crear nueva tarjeta
        recycle_bin_callback: Callback para ir a papelera
    
    Returns:
        ft.Container con el sidebar completo
    """
    return ft.Container(
        content=ft.Column(
            [
                # Header del sidebar
                ft.Container(
                    content=ft.Row(
                        [
                            ft.Icon(ft.Icons.FOLDER, size=theme_manager.icon_size_md, color=theme_manager.primary),
                            ft.Text(
                                "Mis Tarjetas",
                                size=theme_manager.text_size_lg,
                                weight=ft.FontWeight.BOLD,
                                color=theme_manager.text,
                            ),
                        ],
                        spacing=theme_manager.space_12,
                    ),
                    padding=ft.Padding.all(theme_manager.space_20),
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                
                # Búsqueda
                ft.Container(
                    content=search_field,
                    padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12),
                ),
                
                # Lista de tarjetas
                ft.Container(
                    content=cards_listview,
                    padding=ft.Padding.symmetric(horizontal=theme_manager.space_20),
                    expand=True,
                ),
                
                # Footer con contador y botones
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Divider(height=1, color=theme_manager.divider_color),
                            ft.Container(
                                content=card_counter,
                                padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12),
                            ),
                            ft.Container(
                                content=ft.Row(
                                    [
                                        ft.Button(
                                            "Nueva Tarjeta",
                                            icon=ft.Icons.ADD,
                                            on_click=new_card_callback,
                                            expand=True,
                                            style=theme_manager.primary_button_style
                                        ),
                                        ft.IconButton(
                                            icon=ft.Icons.DELETE_OUTLINE,
                                            tooltip="Papelera",
                                            on_click=recycle_bin_callback,
                                        ),
                                        ft.IconButton(
                                            icon=ft.Icons.SETTINGS_OUTLINED,
                                            tooltip="Configuración",
                                            on_click=settings_callback,
                                        ),
                                        ft.IconButton(
                                            icon=ft.Icons.LOGOUT_ROUNDED,
                                            tooltip="Cerrar Sesión",
                                            icon_color=ft.Colors.RED_400,
                                            on_click=logout_callback,
                                        ),
                                    ],
                                    spacing=theme_manager.space_8,
                                ),
                                padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12),
                            ),
                        ],
                        spacing=0,
                    ),
                ),
            ],
            spacing=0,
            expand=True,
        ),
        width=theme_manager.sidebar_width,
        bgcolor=theme_manager.sidebar_bg,
        border=theme_manager.sidebar_border,
    )


def create_card_header(
    selected_card_title: ft.Text, 
    save_indicator: ft.Row,
    edit_callback: Callable,
    delete_callback: Callable,
    lock_callback: Callable
) -> Tuple[ft.Container, ft.IconButton, ft.IconButton, ft.IconButton]:
    """
    Crea el header del panel principal con título, indicador de guardado y acciones.
    Retorna el contenedor y referencias a los botones para control de estado.
    """
    # Botones de acción (Edit/Delete) que se habilitarán/deshabilitarán en Card.py
    edit_button = ft.IconButton(
        icon=ft.Icons.EDIT_OUTLINED,
        tooltip="Cambiar nombre de tarjeta",
        on_click=edit_callback,
        icon_color=theme_manager.primary,
        disabled=True, # Iniciamos deshabilitado
    )
    
    delete_button = ft.IconButton(
        icon=ft.Icons.DELETE_OUTLINE,
        tooltip="Eliminar tarjeta",
        on_click=delete_callback,
        icon_color=ft.Colors.RED_400,
        disabled=True, # Iniciamos deshabilitado
    )

    lock_button = ft.IconButton(
        icon=ft.Icons.LOCK_OUTLINE,
        tooltip="Bloquear tarjeta",
        on_click=lock_callback,
        icon_color=theme_manager.primary,
        disabled=True,
    )

    header_container = ft.Container(
        content=ft.Row(
            [
                ft.Row([selected_card_title, save_indicator], spacing=theme_manager.space_12),
                ft.Row([lock_button, edit_button, delete_button], spacing=theme_manager.space_4),
            ],
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
        ),
        padding=ft.Padding.all(theme_manager.space_20),
    )
    
    return header_container, lock_button, edit_button, delete_button


def create_custom_tabs(
    on_editor_click: Callable,
    on_preview_click: Callable
) -> Tuple[ft.Row, ft.Container, ft.Container]:
    """
    Crea tabs personalizados para Editor/Preview.
    
    Args:
        on_editor_click: Callback para cuando se hace click en tab Editor
        on_preview_click: Callback para cuando se hace click en tab Preview
    
    Returns:
        Tupla con (tabs_row, editor_btn, preview_btn)
    """
    editor_btn = ft.Container(
        content=ft.Text("✏️ Editor", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=ft.Colors.WHITE),
        padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12),
        border_radius=theme_manager.tab_radius,
        bgcolor=theme_manager.primary,
        ink=True,
        on_click=on_editor_click,
    )
    
    preview_btn = ft.Container(
        content=ft.Text("👁️ Lector", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
        padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12),
        border_radius=theme_manager.tab_radius,
        bgcolor=theme_manager.subtle_bg,
        ink=True,
        on_click=on_preview_click,
    )
    
    tabs_row = ft.Row(
        [editor_btn, preview_btn],
        spacing=theme_manager.space_4,
    )
    
    return tabs_row, editor_btn, preview_btn


def create_save_indicator() -> ft.Row:
    """
    Crea indicador visual de guardado.
    
    Returns:
        ft.Row con icono y texto de "Guardado"
    """
    return ft.Row(
        [
            ft.Icon(ft.Icons.CHECK_CIRCLE, color=ft.Colors.GREEN_400, size=theme_manager.icon_size_sm),
            ft.Text("Guardado", size=theme_manager.text_size_sm, color=ft.Colors.GREEN_400),
        ],
        spacing=theme_manager.space_8,
        visible=False,
    )


def create_card_counter() -> ft.Text:
    """
    Crea texto para mostrar contador de tarjetas.
    
    Returns:
        ft.Text para el contador
    """
    return ft.Text(
        "0 tarjetas",
        size=theme_manager.text_size_sm,
        color=theme_manager.subtext,
    )


def create_cards_listview() -> ft.ListView:
    """
    Crea ListView para mostrar las tarjetas.
    
    Returns:
        ft.ListView configurado
    """
    return ft.ListView(
        spacing=theme_manager.space_12,
        padding=ft.Padding.all(0),
        expand=True,
    )
