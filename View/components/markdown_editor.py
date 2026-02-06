"""
Componente de edición Markdown para Flet

Este módulo proporciona funcionalidades para editar y visualizar contenido Markdown:
- Editor de texto con resaltado
- Barra de herramientas con botones de formato
- Vista previa de Markdown
- Funciones helper para manipulación de texto

Uso:
    from View.components.markdown_editor import (
        create_markdown_editor,
        create_markdown_toolbar,
        create_markdown_preview
    )
    
    editor = create_markdown_editor(on_change=my_callback)
    toolbar = create_markdown_toolbar(editor, on_change=my_callback)
    preview = create_markdown_preview()
"""
import flet as ft
from typing import Callable, Optional
from theme.manager import ThemeManager

theme_manager = ThemeManager()


def _find_word_boundaries(text: str, cursor: int) -> tuple:
    """
    Encuentra los límites de la palabra en la posición del cursor.
    
    Args:
        text: Texto completo del editor
        cursor: Posición del cursor
    
    Returns:
        Tupla (start, end) con los índices de inicio y fin de la palabra
    """
    if not text or cursor < 0 or cursor > len(text):
        return (cursor, cursor)
    
    # Expandir hacia la izquierda
    start = cursor
    while start > 0 and (text[start - 1].isalnum() or text[start - 1] in '_-'):
        start -= 1
    
    # Expandir hacia la derecha
    end = cursor
    while end < len(text) and (text[end].isalnum() or text[end] in '_-'):
        end += 1
    
    return (start, end)


def create_markdown_editor(
    on_change: Optional[Callable] = None,
    multiline: bool = True,
    expand: bool = True
) -> ft.TextField:
    """
    Crea un campo de texto para edición de Markdown.
    
    Args:
        on_change: Callback que se llama cuando el contenido cambia
        multiline: Si el editor debe ser multilínea
        expand: Si el editor debe expandirse para llenar el espacio disponible
    
    Returns:
        ft.TextField configurado para edición Markdown
    """
    return ft.TextField(
        value="",
        multiline=multiline,
        min_lines=10,
        max_lines=None,
        expand=expand,
        border_color=ft.Colors.TRANSPARENT,
        focused_border_color=ft.Colors.TRANSPARENT,
        on_change=on_change,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        cursor_color=theme_manager.primary,
        selection_color=ft.Colors.with_opacity(theme_manager.selection_opacity, theme_manager.primary),
    )


def create_markdown_preview() -> ft.Markdown:
    """
    Crea un componente de vista previa de Markdown.
    
    Returns:
        ft.Markdown configurado para vista previa
    """
    return ft.Markdown(
        "",
        selectable=True,
        extension_set=ft.MarkdownExtensionSet.GITHUB_WEB,
        expand=True,
    )


def create_markdown_toolbar(
    editor: ft.TextField,
    on_change: Optional[Callable] = None
) -> ft.Container:
    """
    Crea una barra de herramientas con botones de formato Markdown.
    Usa on_selection_change para rastrear la selección del usuario.
    
    Args:
        editor: El TextField donde se aplicarán los formatos
        on_change: Callback que se llama después de aplicar un formato
    
    Returns:
        ft.Container con la barra de herramientas
    """
    
    # Variables para rastrear la selección actual
    current_selection_start: int = 0
    current_selection_end: int = 0

    def _on_selection_change(e: ft.TextSelectionChangeEvent):
        """Callback que rastrea los cambios de selección"""
        nonlocal current_selection_start, current_selection_end
        if e.selection:
            current_selection_start = e.selection.start if e.selection.start is not None else 0
            current_selection_end = e.selection.end if e.selection.end is not None else 0

    # Conectar el evento de selección
    editor.on_selection_change = _on_selection_change
    
    def _wrap_selection(prefix: str, suffix: str = None):
        """Envuelve el texto seleccionado o inserta en la posición del cursor con prefix/suffix"""
        if suffix is None:
            suffix = prefix
        
        value = editor.value or ""
        
        # Si el editor está vacío, insertar placeholder
        if not value.strip():
            editor.value = f"{prefix}texto{suffix}"
            editor.update()
            if on_change:
                on_change(None)
            return
        
        # Obtener la selección actual
        start = max(0, min(current_selection_start, len(value)))
        end = max(0, min(current_selection_end, len(value)))
        
        if start > end:
            start, end = end, start
        
        # Si hay selección (start != end)
        if start != end:
            # Envolver la selección
            selected = value[start:end]
            new_value = value[:start] + f"{prefix}{selected}{suffix}" + value[end:]
            editor.value = new_value
            editor.update()
            if on_change:
                on_change(None)
        else:
            # Sin selección: insertar placeholder en la posición del cursor
            new_value = value[:start] + f"{prefix}texto{suffix}" + value[start:]
            editor.value = new_value
            editor.update()
            if on_change:
                on_change(None)
    
    def _block_format(prefix: str):
        """Aplica formato de bloque (encabezados, listas, citas)"""
        value = editor.value or ""
        
        if not value.strip():
            # Si está vacío, crear nueva línea con formato
            editor.value = f"{prefix}texto"
        else:
            # Agregar nueva línea con formato al final
            lines = value.split('\n')
            lines.append(f"{prefix}texto")
            editor.value = '\n'.join(lines)
        
        editor.update()
        if on_change:
            on_change(None)
    
    def _insert_text(text: str):
        """Inserta texto en la posición del cursor"""
        value = editor.value or ""
        
        # Intentar obtener posición del cursor
        cursor = len(value)
        if hasattr(editor, 'cursor_position') and editor.cursor_position is not None:
            cursor = editor.cursor_position
        
        # Insertar texto
        new_value = value[:cursor] + text + value[cursor:]
        editor.value = new_value
        editor.update()
        if on_change:
            on_change(None)
    
    # Botones de la barra de herramientas
    toolbar_buttons = [
        ft.IconButton(
            icon=ft.Icons.FORMAT_BOLD,
            tooltip="Negrita",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("**")
        ),
        ft.IconButton(
            icon=ft.Icons.FORMAT_ITALIC,
            tooltip="Cursiva",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("_")
        ),
        ft.IconButton(
            icon=ft.Icons.STRIKETHROUGH_S,
            tooltip="Tachado",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("~~")
        ),
        ft.IconButton(
            icon=ft.Icons.CODE,
            tooltip="Código",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("`"),
            icon_color=theme_manager.text,
        ),
        ft.Container(width=theme_manager.space_1, height=theme_manager.space_20, bgcolor=theme_manager.toolbar_divider_color),
        ft.IconButton(
            icon=ft.Icons.TITLE,
            tooltip="H1",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("# ")
        ),
        ft.IconButton(
            icon=ft.Icons.SUBTITLES,
            tooltip="H2",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("## ")
        ),
        ft.IconButton(
            icon=ft.Icons.TEXT_FIELDS,
            tooltip="H3",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("### "),
            icon_color=theme_manager.text,
        ),
        ft.Container(width=theme_manager.space_1, height=theme_manager.space_20, bgcolor=theme_manager.toolbar_divider_color),
        ft.IconButton(
            icon=ft.Icons.FORMAT_LIST_BULLETED,
            tooltip="Lista",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("- ")
        ),
        ft.IconButton(
            icon=ft.Icons.FORMAT_LIST_NUMBERED,
            tooltip="Lista Nº",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("1. ")
        ),
        ft.IconButton(
            icon=ft.Icons.FORMAT_QUOTE,
            tooltip="Cita",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("> "),
            icon_color=theme_manager.text,
        ),
        ft.Container(width=theme_manager.space_1, height=theme_manager.space_20, bgcolor=theme_manager.toolbar_divider_color),
        ft.IconButton(
            icon=ft.Icons.LINK,
            tooltip="Enlace",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("[", "](https://)")
        ),
        ft.IconButton(
            icon=ft.Icons.IMAGE,
            tooltip="Imagen",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _wrap_selection("![", "](https://)")
        ),
        ft.IconButton(
            icon=ft.Icons.TABLE_CHART,
            tooltip="Tabla",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _insert_text("\n| Col 1 | Col 2 |\n|-------|-------|\n|       |       |\n")
        ),
        ft.IconButton(
            icon=ft.Icons.CHECK_BOX,
            tooltip="Checklist",
            icon_size=theme_manager.icon_size_md,
            on_click=lambda e: _block_format("- [ ] ")
        ),
    ]
    
    return ft.Container(
        content=ft.Row(
            toolbar_buttons,
            wrap=True,
            alignment=ft.MainAxisAlignment.START,
            spacing=theme_manager.space_4,
        ),
        padding=ft.Padding.symmetric(horizontal=theme_manager.space_12, vertical=theme_manager.space_8),
        bgcolor=theme_manager.subtle_bg,
        border_radius=theme_manager.radius_sm,
        visible=True,
    )
