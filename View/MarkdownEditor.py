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
    sel_start: Optional[int] = None
    sel_end: Optional[int] = None

    def _get_selection_from_control(ctrl: ft.TextField) -> Tuple[int, int]:
        # Intentar múltiples propiedades conocidas en Flet/Flutter wrappers
        for start_name, end_name in [
            ("selection_start", "selection_end"),
            ("selection_base_offset", "selection_extent_offset"),
            ("selection_base", "selection_extent"),
        ]:
            s = getattr(ctrl, start_name, None)
            e = getattr(ctrl, end_name, None)
            if isinstance(s, int) and isinstance(e, int):
                return (min(s, e), max(s, e))
        # Fallback: sin selección conocida → seleccionar todo
        value = ctrl.value or ""
        return (0, len(value))

    def _get_selection() -> Tuple[int, int]:
        nonlocal sel_start, sel_end
        # Intentar leer directo del control, si no, usar último conocido
        s, e = _get_selection_from_control(target_field)
        if s is not None and e is not None:
            sel_start, sel_end = s, e
        if sel_start is None or sel_end is None:
            value = target_field.value or ""
            return (0, len(value))
        return (sel_start, sel_end)

    def _set_value_and_notify(new_value: str, new_sel_start: Optional[int] = None, new_sel_end: Optional[int] = None) -> None:
        nonlocal sel_start, sel_end
        target_field.value = new_value
        if isinstance(new_sel_start, int) and isinstance(new_sel_end, int):
            sel_start, sel_end = new_sel_start, new_sel_end
        target_field.update()
        _notify_modified(on_modified)
    def _append(text: str) -> None:
        value = target_field.value or ""
        _set_value_and_notify(value + text)

    def _wrap(prefix: str, suffix: Optional[str] = None) -> None:
        if suffix is None:
            suffix = prefix
        value = target_field.value or ""
        start, end = _get_selection()
        start = max(0, min(start, len(value)))
        end = max(0, min(end, len(value)))
        if start > end:
            start, end = end, start
        selected = value[start:end]
        # Toggle: si ya está envuelto, quitarlo
        if selected.startswith(prefix) and selected.endswith(suffix) and len(selected) >= len(prefix) + len(suffix):
            inner = selected[len(prefix):len(selected) - len(suffix)]
            new_value = value[:start] + inner + value[end:]
            _set_value_and_notify(new_value, start, start + len(inner))
            return
        # Si no hay selección, intentar envolver la palabra en el cursor
        if start == end:
            # Expandir a palabra
            left = start
            right = end
            while left > 0 and value[left - 1].isalnum():
                left -= 1
            while right < len(value) and value[right].isalnum():
                right += 1
            selected = value[left:right]
            start, end = left, right
        new_value = value[:start] + f"{prefix}{selected}{suffix}" + value[end:]
        _set_value_and_notify(new_value, start, start + len(prefix) + len(selected) + len(suffix))

    def _block(prefix: str) -> None:
        value = target_field.value or ""
        start, end = _get_selection()
        start = max(0, min(start, len(value)))
        end = max(0, min(end, len(value)))
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
        _set_value_and_notify(new_value, line_start, line_start + len(new_block))

    def _set(content: str) -> None:
        _set_value_and_notify(content)

    def add_table(cols: int = 3, rows: int = 3) -> None:
        header = " | ".join([f"Col {i+1}" for i in range(cols)])
        sep = " | ".join(["---" for _ in range(cols)])
        body = "\n".join([" | ".join([" ".ljust(3) for _ in range(cols)]) for _ in range(rows)])
        _append(("\n" if (target_field.value or "").strip() else "") + f"{header}\n{sep}\n{body}\n")

    # Botones de barra
    buttons = [
        ft.IconButton(icon=ft.Icons.FORMAT_BOLD, tooltip="Negrita (Ctrl+B)", on_click=lambda e: _wrap("**"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.FORMAT_ITALIC, tooltip="Cursiva (Ctrl+I)", on_click=lambda e: _wrap("*"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.STRIKETHROUGH_S, tooltip="Tachado (Alt+Shift+S)", on_click=lambda e: _wrap("~~"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(icon=ft.Icons.CODE, tooltip="Código en línea (Ctrl+`)", on_click=lambda e: _wrap("`"), icon_size=theme_manager.icon_size_md),
        ft.IconButton(
            icon=ft.Icons.CODE,
            tooltip="Bloque de código",
            on_click=lambda e: _wrap("```\n", "\n```"),
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
            on_click=lambda e: _append("[texto](https://)"),
            icon_size=theme_manager.icon_size_md,
        ),
        ft.IconButton(
            icon=ft.Icons.IMAGE,
            tooltip="Imagen",
            on_click=lambda e: _append("![alt](https://)"),
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
            on_click=lambda e: _append("\n- [ ] tarea\n"),
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
