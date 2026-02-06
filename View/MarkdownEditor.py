import flet as ft
from typing import Callable, Optional, Tuple
import asyncio
from theme.manager import ThemeManager

theme_manager = ThemeManager()

def _notify_modified(on_modified: Optional[Callable[[], None]]) -> None:
    if on_modified is not None:
        try:
            if asyncio.iscoroutinefunction(on_modified):
                asyncio.create_task(on_modified())
            else:
                on_modified()
        except Exception:
            pass


def create_markdown_preview() -> ft.Markdown:
    return ft.Markdown(
        value="",
        selectable=True,
        extension_set=ft.MarkdownExtensionSet.GITHUB_WEB,
        code_theme=ft.MarkdownCodeTheme.GITHUB,
        auto_follow_links=True,
        shrink_wrap=False,
        fit_content=False,
    )


def create_markdown_toolbar(
    target_field: ft.TextField,
    on_modified: Optional[Callable[[], None]] = None,
    show_code_switch: Optional[ft.Switch] = None,
) -> ft.Container:
    """
    Crea una barra de herramientas para formato Markdown.
    Usa on_selection_change para rastrear la selección del usuario.
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
    target_field.on_selection_change = _on_selection_change

    def _set_value_and_notify(new_value: str) -> None:
        target_field.value = new_value
        target_field.update()
        _notify_modified(on_modified)

    def _append(text: str) -> None:
        value = target_field.value or ""
        _set_value_and_notify(value + text)

    def _wrap(prefix: str, suffix: Optional[str] = None) -> None:
        """Envuelve el texto seleccionado o inserta placeholder en la posición del cursor"""
        if suffix is None:
            suffix = prefix
        
        value = target_field.value or ""
        start = max(0, min(current_selection_start, len(value)))
        end = max(0, min(current_selection_end, len(value)))
        
        if start > end:
            start, end = end, start
        
        # Si hay selección (start != end)
        if start != end:
            selected = value[start:end]
            # Toggle: si ya está envuelto, quitarlo
            if selected.startswith(prefix) and selected.endswith(suffix) and len(selected) >= len(prefix) + len(suffix):
                inner = selected[len(prefix):len(selected) - len(suffix)]
                new_value = value[:start] + inner + value[end:]
                _set_value_and_notify(new_value)
                return
            # Envolver la selección
            new_value = value[:start] + f"{prefix}{selected}{suffix}" + value[end:]
            _set_value_and_notify(new_value)
        else:
            # Sin selección: insertar placeholder en la posición del cursor
            placeholder = "texto"
            new_value = value[:start] + f"{prefix}{placeholder}{suffix}" + value[start:]
            _set_value_and_notify(new_value)

    def _block(prefix: str) -> None:
        """Aplica formato de bloque a las líneas seleccionadas"""
        value = target_field.value or ""
        start = max(0, min(current_selection_start, len(value)))
        end = max(0, min(current_selection_end, len(value)))
        
        if start > end:
            start, end = end, start
        
        # Expandir selección a límites de línea
        line_start = value.rfind("\n", 0, start) + 1
        line_end = value.find("\n", end)
        if line_end == -1:
            line_end = len(value)
        
        block = value[line_start:line_end]
        lines = block.splitlines() if block else [""]
        
        # Toggle: si todas las líneas ya tienen el prefijo, quitarlo; si no, agregar
        if all(l.startswith(prefix) or not l.strip() for l in lines):
            new_lines = [l[len(prefix):] if l.startswith(prefix) else l for l in lines]
        else:
            new_lines = [f"{prefix}{l}" if l.strip() else l for l in lines]
        
        new_block = "\n".join(new_lines)
        new_value = value[:line_start] + new_block + value[line_end:]
        _set_value_and_notify(new_value)

    def add_table(cols: int = 3, rows: int = 3) -> None:
        header = " | ".join([f"Col {i+1}" for i in range(cols)])
        sep = " | ".join(["---" for _ in range(cols)])
        body = "\n".join([" | ".join([" ".ljust(3) for _ in range(cols)]) for _ in range(rows)])
        _append(("\n" if (target_field.value or "").strip() else "") + f"{header}\n{sep}\n{body}\n")

    # Botones de barra
    buttons = [
        ft.IconButton(icon=ft.Icons.FORMAT_BOLD, tooltip="Negrita (Ctrl+B)", on_click=lambda e: _wrap("**"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.FORMAT_ITALIC, tooltip="Cursiva (Ctrl+I)", on_click=lambda e: _wrap("_"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.STRIKETHROUGH_S, tooltip="Tachado (Alt+Shift+S)", on_click=lambda e: _wrap("~~"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.CODE, tooltip="Código en línea (Ctrl+`)", on_click=lambda e: _wrap("`"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(
            icon=ft.Icons.CODE_OFF,
            tooltip="Bloque de código",
            on_click=lambda e: _block("```\n"),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.VerticalDivider(width=theme_manager.space_12, color=ft.Colors.TRANSPARENT),
        ft.IconButton(icon=ft.Icons.TITLE, tooltip="Encabezado H1", on_click=lambda e: _block("# "), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.SUBTITLES, tooltip="Encabezado H2", on_click=lambda e: _block("## "), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.TEXT_FIELDS, tooltip="Encabezado H3", on_click=lambda e: _block("### "), icon_size=theme_manager.icon_size_md),
        ft.VerticalDivider(width=theme_manager.space_12, color=ft.Colors.TRANSPARENT),
        ft.IconButton(icon=ft.Icons.FORMAT_LIST_BULLETED, tooltip="Lista", on_click=lambda e: _block("- "), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.FORMAT_LIST_NUMBERED, tooltip="Lista numerada", on_click=lambda e: _block("1. "), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.FORMAT_QUOTE, tooltip="Cita", on_click=lambda e: _block("> "), icon_size=theme_manager.icon_size_md),
        ft.VerticalDivider(width=theme_manager.space_12, color=ft.Colors.TRANSPARENT),
        ft.IconButton(
            icon=ft.Icons.LINK,
            tooltip="Enlace",
            on_click=lambda e: _wrap("[", "](https://)"),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.IconButton(
            icon=ft.Icons.IMAGE,
            tooltip="Imagen",
            on_click=lambda e: _wrap("![", "](https://)"),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.IconButton(
            icon=ft.Icons.TABLE_CHART,
            tooltip="Tabla",
            on_click=lambda e: add_table(),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.IconButton(
            icon=ft.Icons.HORIZONTAL_RULE,
            tooltip="Regla horizontal",
            on_click=lambda e: _append("\n---\n"),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.IconButton(
            icon=ft.Icons.CHECK_BOX,
            tooltip="Checklist",
            on_click=lambda e: _block("- [ ] "),
            icon_size=theme_manager.icon_size_md,
        ),
    ]

    # Integrar el switch "Mostrar código" dentro de la barra si se proporciona
    if show_code_switch is not None:
        label = ft.Text("Mostrar código", size=theme_manager.text_size_sm, color=theme_manager.subtext)
        buttons.append(ft.VerticalDivider(width=theme_manager.space_12, color=ft.Colors.TRANSPARENT))
        buttons.append(label)
        buttons.append(show_code_switch)

    bar = ft.Container(
        content=ft.Row(controls=buttons, wrap=True, alignment=ft.MainAxisAlignment.START, spacing=theme_manager.space_8),
        padding=ft.Padding.symmetric(vertical=theme_manager.space_8, horizontal=theme_manager.space_8),
        bgcolor=theme_manager.subtle_bg,
        border=theme_manager.card_border,
        border_radius=theme_manager.radius_sm,
        visible=True,
    )
    return bar

