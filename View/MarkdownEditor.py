import flet as ft
from typing import Callable, Optional


def _notify_modified(on_modified: Optional[Callable[[], None]]) -> None:
    if on_modified is not None:
        try:
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
) -> ft.Container:
    def _append(text: str) -> None:
        target_field.value = (target_field.value or "") + text
        target_field.update()
        _notify_modified(on_modified)

    def _wrap(prefix: str, suffix: Optional[str] = None) -> None:
        if suffix is None:
            suffix = prefix
        content = target_field.value or ""
        target_field.value = f"{prefix}{content}{suffix}"
        target_field.update()
        _notify_modified(on_modified)

    def _block(prefix: str) -> None:
        content = target_field.value or ""
        lines = content.splitlines() if content else [""]
        lines = [f"{prefix}{line}" if line.strip() else line for line in lines]
        target_field.value = "\n".join(lines)
        target_field.update()
        _notify_modified(on_modified)

    def _set(content: str) -> None:
        target_field.value = content
        target_field.update()
        _notify_modified(on_modified)

    def add_table(cols: int = 3, rows: int = 3) -> None:
        header = " | ".join([f"Col {i+1}" for i in range(cols)])
        sep = " | ".join(["---" for _ in range(cols)])
        body = "\n".join([" | ".join([" ".ljust(3) for _ in range(cols)]) for _ in range(rows)])
        _append(("\n" if (target_field.value or "").strip() else "") + f"{header}\n{sep}\n{body}\n")

    # Botones de barra
    buttons = [
        ft.IconButton(icon=ft.Icons.FORMAT_BOLD, tooltip="Negrita", on_click=lambda e: _wrap("**")),
        ft.IconButton(icon=ft.Icons.FORMAT_ITALIC, tooltip="Cursiva", on_click=lambda e: _wrap("*")),
        ft.IconButton(icon=ft.Icons.STRIKETHROUGH_S, tooltip="Tachado", on_click=lambda e: _wrap("~~")),
        ft.IconButton(icon=ft.Icons.CODE, tooltip="Código en línea", on_click=lambda e: _wrap("`")),
        ft.IconButton(
            icon=ft.Icons.CODE,
            tooltip="Bloque de código",
            on_click=lambda e: _wrap("```\n", "\n```"),
        ),
        ft.VerticalDivider(width=10, color=ft.Colors.TRANSPARENT),
        ft.IconButton(icon=ft.Icons.TITLE, tooltip="Encabezado H1", on_click=lambda e: _block("# ")),
        ft.IconButton(icon=ft.Icons.SUBTITLES, tooltip="Encabezado H2", on_click=lambda e: _block("## ")),
        ft.IconButton(icon=ft.Icons.TEXT_FIELDS, tooltip="Encabezado H3", on_click=lambda e: _block("### ")),
        ft.VerticalDivider(width=10, color=ft.Colors.TRANSPARENT),
        ft.IconButton(icon=ft.Icons.FORMAT_LIST_BULLETED, tooltip="Lista", on_click=lambda e: _block("- ")),
        ft.IconButton(icon=ft.Icons.FORMAT_LIST_NUMBERED, tooltip="Lista numerada", on_click=lambda e: _block("1. ")),
        ft.IconButton(icon=ft.Icons.FORMAT_QUOTE, tooltip="Cita", on_click=lambda e: _block("> ")),
        ft.VerticalDivider(width=10, color=ft.Colors.TRANSPARENT),
        ft.IconButton(
            icon=ft.Icons.LINK,
            tooltip="Enlace",
            on_click=lambda e: _append("[texto](https://)")
        ),
        ft.IconButton(
            icon=ft.Icons.IMAGE,
            tooltip="Imagen",
            on_click=lambda e: _append("![alt](https://)")
        ),
        ft.IconButton(
            icon=ft.Icons.TABLE_CHART,
            tooltip="Tabla",
            on_click=lambda e: add_table(),
        ),
        ft.IconButton(
            icon=ft.Icons.HORIZONTAL_RULE,
            tooltip="Regla horizontal",
            on_click=lambda e: _append("\n---\n"),
        ),
        ft.IconButton(
            icon=ft.Icons.CHECK_BOX,
            tooltip="Checklist",
            on_click=lambda e: _append("\n- [ ] tarea\n"),
        ),
    ]

    bar = ft.Container(
        content=ft.Row(controls=buttons, wrap=True, alignment=ft.MainAxisAlignment.START, spacing=6),
        padding=ft.padding.symmetric(8, 8),
        bgcolor=ft.Colors.SURFACE,
        border=ft.border.all(1, ft.Colors.BLUE_200),
        border_radius=6,
        visible=True,
    )
    return bar


