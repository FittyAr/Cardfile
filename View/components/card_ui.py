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
        border_color=ft.Colors.OUTLINE,
        on_change=on_change,
        text_size=14,
    )


def create_sidebar(
    search_field: ft.TextField,
    cards_listview: ft.ListView,
    card_counter: ft.Text,
    new_card_callback: Callable,
    recycle_bin_callback: Callable
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
                            ft.Icon(ft.Icons.FOLDER, size=24, color=ft.Colors.BLUE_400),
                            ft.Text(
                                "Mis Tarjetas",
                                size=18,
                                weight=ft.FontWeight.BOLD,
                                color=ft.Colors.ON_SURFACE,
                            ),
                        ],
                        spacing=12,
                    ),
                    padding=ft.Padding.all(20),
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                # Búsqueda
                ft.Container(
                    content=search_field,
                    padding=ft.Padding.symmetric(horizontal=20, vertical=10),
                ),
                
                # Lista de tarjetas
                ft.Container(
                    content=cards_listview,
                    padding=ft.Padding.symmetric(horizontal=20),
                    expand=True,
                ),
                
                # Footer con contador y botones
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                            ft.Container(
                                content=card_counter,
                                padding=ft.Padding.symmetric(horizontal=20, vertical=10),
                            ),
                            ft.Container(
                                content=ft.Row(
                                    [
                                        ft.ElevatedButton(
                                            "Nueva Tarjeta",
                                            icon=ft.Icons.ADD,
                                            on_click=new_card_callback,
                                            expand=True,
                                        ),
                                        ft.IconButton(
                                            icon=ft.Icons.DELETE_OUTLINE,
                                            tooltip="Papelera",
                                            on_click=recycle_bin_callback,
                                        ),
                                    ],
                                    spacing=8,
                                ),
                                padding=ft.Padding.symmetric(horizontal=20, vertical=10),
                            ),
                        ],
                        spacing=0,
                    ),
                ),
            ],
            spacing=0,
            expand=True,
        ),
        width=320,
        bgcolor=ft.Colors.SURFACE,
        border=ft.border.only(right=ft.BorderSide(1, ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE))),
    )


def create_card_header(
    selected_card_title: ft.Text, 
    save_indicator: ft.Row,
    edit_callback: Callable,
    delete_callback: Callable
) -> Tuple[ft.Container, ft.IconButton, ft.IconButton]:
    """
    Crea el header del panel principal con título, indicador de guardado y acciones.
    Retorna el contenedor y referencias a los botones para control de estado.
    """
    # Botones de acción (Edit/Delete) que se habilitarán/deshabilitarán en Card.py
    edit_button = ft.IconButton(
        icon=ft.Icons.EDIT_OUTLINED,
        tooltip="Editar título",
        on_click=edit_callback,
        icon_color=ft.Colors.BLUE_400,
        disabled=True, # Iniciamos deshabilitado
    )
    
    delete_button = ft.IconButton(
        icon=ft.Icons.DELETE_OUTLINE,
        tooltip="Eliminar tarjeta",
        on_click=delete_callback,
        icon_color=ft.Colors.RED_400,
        disabled=True, # Iniciamos deshabilitado
    )

    header_container = ft.Container(
        content=ft.Row(
            [
                ft.Row([selected_card_title, save_indicator], spacing=10),
                ft.Row([edit_button, delete_button], spacing=4),
            ],
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
        ),
        padding=ft.Padding.all(20),
    )
    
    return header_container, edit_button, delete_button


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
        content=ft.Text("✏️ Editor", size=14, weight=ft.FontWeight.W_600, color=ft.Colors.WHITE),
        padding=ft.Padding.symmetric(horizontal=20, vertical=12),
        border_radius=ft.border_radius.only(top_left=8, top_right=8),
        bgcolor=ft.Colors.BLUE_400,
        ink=True,
        on_click=on_editor_click,
    )
    
    preview_btn = ft.Container(
        content=ft.Text("👁️ Vista Previa", size=14, weight=ft.FontWeight.W_600),
        padding=ft.Padding.symmetric(horizontal=20, vertical=12),
        border_radius=ft.border_radius.only(top_left=8, top_right=8),
        bgcolor=ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE),
        ink=True,
        on_click=on_preview_click,
    )
    
    tabs_row = ft.Row(
        [editor_btn, preview_btn],
        spacing=4,
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
            ft.Icon(ft.Icons.CHECK_CIRCLE, color=ft.Colors.GREEN_400, size=16),
            ft.Text("Guardado", size=12, color=ft.Colors.GREEN_400),
        ],
        spacing=6,
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
        size=12,
        color=ft.Colors.with_opacity(0.6, ft.Colors.ON_SURFACE),
    )


def create_cards_listview() -> ft.ListView:
    """
    Crea ListView para mostrar las tarjetas.
    
    Returns:
        ft.ListView configurado
    """
    return ft.ListView(
        spacing=12,
        padding=ft.Padding.all(0),
        expand=True,
    )
