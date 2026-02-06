import flet as ft
from typing import Callable
from config.config import Config
from config.runtime import is_web_runtime
from config.security import normalize_allowed_ips
from theme.manager import ThemeManager
from theme.colors import ThemeColors

theme_manager = ThemeManager()


async def settings_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    config = Config()
    is_web = is_web_runtime(page)
    client_ip = getattr(page, "client_ip", None) if is_web else None
    run_mode = "Web" if is_web else "Escritorio"

    language_options = [
        ft.DropdownOption(opt["value"], opt["text"])
        for opt in config.get_language_options()
    ]
    theme_options = [
        ft.DropdownOption(name, ThemeColors.THEMES[name]["name"])
        for name in ThemeColors.THEMES
    ]

    theme_dd = ft.Dropdown(
        options=theme_options,
        value=theme_manager.current_theme_name,
        width=theme_manager.input_width,
    )

    language_dd = ft.Dropdown(
        options=language_options,
        value=config.current_language,
        width=theme_manager.input_width,
    )

    require_login_switch = ft.Switch(
        value=config.get("app.auth.require_login", True)
    )

    session_days_field = ft.TextField(
        value=str(config.get("app.auth.session_expiry_days", 7)),
        keyboard_type=ft.KeyboardType.NUMBER,
        width=theme_manager.input_width,
    )

    debug_switch = ft.Switch(
        value=config.get("app.debug", False)
    )

    allowed_ips_field = None
    if is_web:
        allowed_ips = config.get("app.web.allowed_ips", ["0.0.0.0"])
        allowed_ips_field = ft.TextField(
            value=", ".join(normalize_allowed_ips(allowed_ips)),
            multiline=True,
            min_lines=2,
            max_lines=4,
            width=theme_manager.input_width,
        )

    async def save_clicked(e):
        selected_theme = theme_dd.value or theme_manager.current_theme_name
        theme_manager.set_theme(selected_theme)

        selected_language = language_dd.value or config.current_language
        if selected_language != config.current_language:
            config.set_language(selected_language)

        config.set("app.auth.require_login", require_login_switch.value)
        config.set("app.debug", debug_switch.value)

        try:
            session_days = int(session_days_field.value)
            if session_days > 0:
                config.set("app.auth.session_expiry_days", session_days)
        except Exception:
            pass

        if is_web and allowed_ips_field:
            config.set("app.web.allowed_ips", normalize_allowed_ips(allowed_ips_field.value))

        await on_success()

    async def cancel_clicked(e):
        await on_close()

    general_section = ft.Container(
        content=ft.Column(
            [
                ft.Text("Tema", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
                theme_dd,
                ft.Container(height=theme_manager.space_8),
                ft.Text("Idioma predeterminado", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
                language_dd,
            ],
            spacing=theme_manager.space_12,
            horizontal_alignment=ft.CrossAxisAlignment.STRETCH,
        ),
        visible=True,
    )

    security_items = [
        ft.Text("Autenticación", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
        ft.Row(
            [
                ft.Text("Requerir inicio de sesión", color=theme_manager.subtext),
                require_login_switch,
            ],
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
        ),
        ft.Row(
            [
                ft.Text("Duración de sesión (días)", color=theme_manager.subtext),
                session_days_field,
            ],
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
        ),
    ]

    if is_web and allowed_ips_field:
        security_items.extend(
            [
                ft.Container(height=theme_manager.space_8),
                ft.Text("Seguridad IP", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
                ft.Text(f"IP actual: {client_ip or 'Desconocida'}", color=theme_manager.subtext),
                ft.Text("IPs permitidas (separadas por coma o líneas)", color=theme_manager.subtext),
                allowed_ips_field,
            ]
        )

    security_section = ft.Container(
        content=ft.Column(
            security_items,
            spacing=theme_manager.space_12,
            horizontal_alignment=ft.CrossAxisAlignment.STRETCH,
        ),
        visible=False,
    )

    system_section = ft.Container(
        content=ft.Column(
            [
                ft.Text("Sistema", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
                ft.Text(f"Modo de ejecución: {run_mode}", color=theme_manager.subtext),
                ft.Container(height=theme_manager.space_8),
                ft.Row(
                    [
                        ft.Text("Modo debug", color=theme_manager.subtext),
                        debug_switch,
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
            ],
            spacing=theme_manager.space_12,
            horizontal_alignment=ft.CrossAxisAlignment.STRETCH,
        ),
        visible=False,
    )

    sections = {
        "general": general_section,
        "security": security_section,
        "system": system_section,
    }

    menu_items = {}

    def set_section(key):
        for section_key, container in sections.items():
            container.visible = section_key == key
        for item_key, item in menu_items.items():
            selected = item_key == key
            item["container"].bgcolor = theme_manager.subtle_bg if selected else None
            item["icon"].color = theme_manager.primary if selected else theme_manager.subtext
            item["text"].color = theme_manager.primary if selected else theme_manager.subtext
            item["text"].weight = ft.FontWeight.BOLD if selected else ft.FontWeight.W_600
        page.update()

    def build_menu_item(key, label, icon):
        icon_control = ft.Icon(icon, color=theme_manager.subtext, size=theme_manager.icon_size_md)
        text_control = ft.Text(label, color=theme_manager.subtext, size=theme_manager.text_size_md, weight=ft.FontWeight.W_600)
        container = ft.Container(
            content=ft.Row([icon_control, text_control], spacing=theme_manager.space_12),
            padding=ft.Padding.symmetric(horizontal=theme_manager.space_16, vertical=theme_manager.space_12),
            border_radius=theme_manager.radius_md,
            ink=True,
            on_click=lambda e, k=key: set_section(k),
        )
        menu_items[key] = {"container": container, "icon": icon_control, "text": text_control}
        return container

    menu = ft.Column(
        [
            ft.Text("Configuración", size=theme_manager.text_size_lg, weight=ft.FontWeight.BOLD, color=theme_manager.text),
            ft.Container(height=theme_manager.space_8),
            build_menu_item("general", "General", ft.Icons.TUNE),
            build_menu_item("security", "Seguridad", ft.Icons.SHIELD_OUTLINED),
            build_menu_item("system", "Sistema", ft.Icons.SETTINGS_APPLICATIONS),
        ],
        spacing=theme_manager.space_4,
        expand=True,
    )

    set_section("general")

    content_stack = ft.Column(
        [
            general_section,
            security_section,
            system_section,
        ],
        spacing=theme_manager.space_12,
        expand=True,
    )

    right_panel = ft.Column(
        [
            ft.Row(
                [
                    ft.Icon(ft.Icons.SETTINGS_OUTLINED, color=theme_manager.primary, size=theme_manager.icon_size_lg),
                    ft.Text("Configuración", size=theme_manager.text_size_xxl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                ],
                alignment=ft.MainAxisAlignment.START,
                spacing=theme_manager.space_12,
            ),
            ft.Divider(height=1, color=theme_manager.divider_color),
            ft.Container(
                content=ft.ListView(
                    controls=[content_stack],
                    spacing=theme_manager.space_12,
                    expand=True,
                ),
                expand=True,
                padding=ft.Padding.only(right=theme_manager.space_8),
            ),
            ft.Container(
                content=ft.Row(
                    [
                        ft.TextButton(
                            content=ft.Text("Cancelar", color=ft.Colors.RED_400),
                            on_click=cancel_clicked,
                        ),
                        ft.Button(
                            content=ft.Text("Guardar", weight=ft.FontWeight.BOLD),
                            width=theme_manager.button_width,
                            height=theme_manager.button_height,
                            color=ft.Colors.WHITE,
                            bgcolor=theme_manager.primary,
                            style=theme_manager.primary_button_style,
                            on_click=save_clicked,
                        ),
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                padding=ft.Padding.only(top=theme_manager.space_12),
            ),
        ],
        spacing=theme_manager.space_12,
        expand=True,
    )

    return ft.Container(
        content=ft.Row(
            [
                ft.Container(
                    content=menu,
                    width=230,
                    bgcolor=theme_manager.subtle_bg,
                    border_radius=theme_manager.radius_md,
                    padding=ft.Padding.all(theme_manager.space_12),
                ),
                ft.Container(
                    content=right_panel,
                    expand=True,
                    padding=ft.Padding.all(theme_manager.space_12),
                ),
            ],
            spacing=theme_manager.space_12,
            expand=True,
        ),
        width=900,
        height=620,
        padding=theme_manager.modal_padding,
        bgcolor=theme_manager.card_bg,
        border_radius=theme_manager.radius_lg,
        border=theme_manager.card_border,
        shadow=theme_manager.card_shadow,
        alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )


__all__ = ["settings_modal"]
