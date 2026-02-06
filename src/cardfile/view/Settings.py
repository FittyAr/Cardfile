import flet as ft
from typing import Callable
from cardfile.config.config import Config
from cardfile.config.locking import get_user_locking_settings, hash_lock_password, verify_lock_password
from cardfile.config.runtime import is_web_runtime
from cardfile.config.security import normalize_allowed_ips
from cardfile.theme.manager import ThemeManager
from cardfile.theme.colors import ThemeColors
from cardfile.data.database.connection import get_session
from cardfile.data.models.usuario import Usuario
from cardfile.view.components.auth_manager import AuthManager

theme_manager = ThemeManager()


async def settings_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    config = Config()
    is_web = is_web_runtime(page)
    client_ip = getattr(page, "client_ip", None) if is_web else None
    run_mode = "Web" if is_web else "Escritorio"
    auth_manager = AuthManager(page)
    user_id = await auth_manager.get_authenticated_user_id()
    session = get_session()
    try:
        current_user = session.query(Usuario).filter(Usuario.id == user_id).first()
    finally:
        session.close()
    locking_settings = get_user_locking_settings(config, current_user)

    initial_locking_enabled = locking_settings["enabled"]
    default_allowed_ips = ", ".join(normalize_allowed_ips(config.get("app.web.allowed_ips", ["0.0.0.0"]))) if is_web else ""

    theme_dd = None
    language_dd = None
    require_login_switch = None
    session_days_field = None
    debug_switch = None
    locking_enabled_switch = None
    locking_password_field = None
    locking_password_hint = None
    locking_timeout_field = None
    locking_mask_field = None
    allowed_ips_field = None
    disable_password_field = None
    disable_password_overlay = None
    root_container = None

    pending_password_value = ""
    pending_has_password_hash = False

    def snapshot_state():
        return {
            "theme": theme_dd.value if theme_dd else theme_manager.current_theme_name,
            "language": language_dd.value if language_dd else config.current_language,
            "require_login": require_login_switch.value if require_login_switch else config.get("app.auth.require_login", True),
            "session_days": session_days_field.value if session_days_field else str(config.get("app.auth.session_expiry_days", 7)),
            "debug": debug_switch.value if debug_switch else config.get("app.debug", False),
            "locking_enabled": locking_enabled_switch.value if locking_enabled_switch else locking_settings["enabled"],
            "locking_password": locking_password_field.value if locking_password_field else "",
            "locking_timeout": locking_timeout_field.value if locking_timeout_field else str(locking_settings["auto_lock_seconds"]),
            "locking_mask": locking_mask_field.value if locking_mask_field else str(locking_settings["mask_visible_chars"]),
            "allowed_ips": allowed_ips_field.value if allowed_ips_field else default_allowed_ips,
            "disable_password": disable_password_field.value if disable_password_field else "",
            "disable_overlay_visible": disable_password_overlay.visible if disable_password_overlay else False,
        }

    def apply_theme_preview(e):
        state = snapshot_state()
        selected_theme = state["theme"] or theme_manager.current_theme_name
        theme_manager.preview_theme(selected_theme)
        page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
        root_container.content = build_ui(state)
        root_container.bgcolor = theme_manager.card_bg
        root_container.border = theme_manager.card_border
        root_container.shadow = theme_manager.card_shadow
        root_container.padding = theme_manager.modal_padding
        root_container.border_radius = theme_manager.radius_lg
        page.update()

    async def confirm_disable_clicked(e):
        nonlocal pending_password_value, pending_has_password_hash
        entered_password = (disable_password_field.value or "").strip()
        if not entered_password:
            page.show_dialog(ft.SnackBar(
                content=ft.Text("Debes ingresar la contraseña"),
                bgcolor=ft.Colors.RED_400,
                action="Ok",
                duration=2000
            ))
            page.update()
            return
        if not verify_lock_password(entered_password, locking_settings["password_hash"]):
            page.show_dialog(ft.SnackBar(
                content=ft.Text("Contraseña incorrecta"),
                bgcolor=ft.Colors.RED_400,
                action="Ok",
                duration=2000
            ))
            page.update()
            return
        disable_password_field.value = ""
        disable_password_overlay.visible = False
        page.update()
        await persist_lock_settings(pending_password_value, pending_has_password_hash)

    def cancel_disable_clicked(e):
        disable_password_field.value = ""
        disable_password_overlay.visible = False
        page.update()

    async def save_clicked(e):
        selected_theme = theme_dd.value or theme_manager.current_theme_name
        theme_manager.set_theme(selected_theme)
        page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT

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

        password_value = (locking_password_field.value or "").strip()
        has_password_hash = bool(locking_settings["password_hash"])

        if initial_locking_enabled and not locking_enabled_switch.value and has_password_hash:
            pending_password_value = password_value
            pending_has_password_hash = has_password_hash
            disable_password_overlay.visible = True
            page.update()
            return

        await persist_lock_settings(password_value, has_password_hash)

    async def persist_lock_settings(password_value, has_password_hash):

        session = get_session()
        try:
            user = session.query(Usuario).filter(Usuario.id == user_id).first()
            if user:
                if locking_enabled_switch.value:
                    if not password_value and not has_password_hash:
                        page.show_dialog(ft.SnackBar(
                            content=ft.Text("Debes definir una contraseña de tarjetas"),
                            bgcolor=ft.Colors.RED_400,
                            action="Ok",
                            duration=2000
                        ))
                        page.update()
                        return
                user.locking_enabled = locking_enabled_switch.value
                if password_value and locking_enabled_switch.value:
                    user.locking_password_hash = hash_lock_password(password_value)
                try:
                    auto_lock_seconds = int(locking_timeout_field.value)
                    user.locking_auto_lock_seconds = max(auto_lock_seconds, 0)
                except Exception:
                    pass
                try:
                    mask_chars = int(locking_mask_field.value)
                    user.locking_mask_visible_chars = max(mask_chars, 0)
                except Exception:
                    pass
                session.commit()
        finally:
            session.close()

        if is_web and allowed_ips_field:
            config.set("app.web.allowed_ips", normalize_allowed_ips(allowed_ips_field.value))

        await on_success()

    async def cancel_clicked(e):
        await on_close()

    def build_ui(state):
        nonlocal theme_dd, language_dd, require_login_switch, session_days_field, debug_switch
        nonlocal locking_enabled_switch, locking_password_field, locking_password_hint
        nonlocal locking_timeout_field, locking_mask_field, allowed_ips_field
        nonlocal disable_password_field, disable_password_overlay

        language_options = [
            ft.DropdownOption(opt["value"], opt["text"])
            for opt in config.get_language_options()
        ]
        theme_options = [
            ft.DropdownOption(
                name,
                f'{ThemeColors.THEMES[name]["name"]} ({"Oscuro" if ThemeColors.THEMES[name]["is_dark"] else "Claro"})'
            )
            for name in ThemeColors.THEMES
        ]

        theme_dd = ft.Dropdown(
            options=theme_options,
            value=state["theme"],
            width=theme_manager.input_width,
            on_select=apply_theme_preview,
        )

        language_dd = ft.Dropdown(
            options=language_options,
            value=state["language"],
            width=theme_manager.input_width,
        )

        require_login_switch = ft.Switch(
            value=state["require_login"]
        )

        session_days_field = ft.TextField(
            value=str(state["session_days"]),
            keyboard_type=ft.KeyboardType.NUMBER,
            width=theme_manager.input_width,
        )

        debug_switch = ft.Switch(
            value=state["debug"]
        )

        locking_enabled_switch = ft.Switch(
            value=state["locking_enabled"]
        )
        locking_password_field = ft.TextField(
            label="Contraseña de tarjetas",
            password=True,
            can_reveal_password=True,
            hint_text="Contraseña guardada" if locking_settings["password_hash"] else "",
            width=theme_manager.input_width,
            value=state["locking_password"],
        )
        locking_password_hint = ft.Text(
            "Ya existe una contraseña guardada",
            color=theme_manager.subtext,
            size=theme_manager.text_size_sm,
            visible=bool(locking_settings["password_hash"]),
        )
        locking_timeout_field = ft.TextField(
            value=str(state["locking_timeout"]),
            keyboard_type=ft.KeyboardType.NUMBER,
            width=theme_manager.input_width,
        )
        locking_mask_field = ft.TextField(
            value=str(state["locking_mask"]),
            keyboard_type=ft.KeyboardType.NUMBER,
            width=theme_manager.input_width,
        )

        allowed_ips_field = None
        if is_web:
            allowed_ips_field = ft.TextField(
                value=state["allowed_ips"],
                multiline=True,
                min_lines=2,
                max_lines=4,
                width=theme_manager.input_width,
            )

        disable_password_field = ft.TextField(
            label="Contraseña de tarjetas",
            password=True,
            can_reveal_password=True,
            width=theme_manager.input_width,
            value=state["disable_password"],
        )
        disable_password_overlay = ft.Container(visible=state["disable_overlay_visible"])

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
            ft.Container(height=theme_manager.space_8),
            ft.Text("Bloqueo de tarjetas", size=theme_manager.text_size_md, weight=ft.FontWeight.W_600, color=theme_manager.text),
            ft.Row(
                [
                    ft.Text("Habilitar bloqueo", color=theme_manager.subtext),
                    locking_enabled_switch,
                ],
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            ),
            locking_password_field,
            locking_password_hint,
            ft.Row(
                [
                    ft.Text("Tiempo de bloqueo (seg)", color=theme_manager.subtext),
                    locking_timeout_field,
                ],
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            ),
            ft.Row(
                [
                    ft.Text("Caracteres visibles", color=theme_manager.subtext),
                    locking_mask_field,
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

        main_content = ft.Row(
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
        )

        disable_password_overlay.content = ft.Container(
            content=ft.Column(
                [
                    ft.Row(
                        [
                            ft.Icon(ft.Icons.LOCK_OUTLINED, color=theme_manager.primary, size=theme_manager.icon_size_lg),
                            ft.Text("Confirmar deshabilitado", size=theme_manager.text_size_xl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                        ],
                        alignment=ft.MainAxisAlignment.CENTER,
                        spacing=theme_manager.space_12,
                    ),
                    ft.Divider(height=1, color=theme_manager.divider_color),
                    ft.Text("Confirma la contraseña para deshabilitar el bloqueo", color=theme_manager.subtext),
                    disable_password_field,
                    ft.Row(
                        [
                            ft.TextButton(
                                content=ft.Text("Cancelar", color=ft.Colors.RED_400),
                                on_click=cancel_disable_clicked,
                            ),
                            ft.Button(
                                content=ft.Text("Confirmar", weight=ft.FontWeight.BOLD),
                                width=theme_manager.button_width,
                                height=theme_manager.button_height,
                                color=ft.Colors.WHITE,
                                bgcolor=theme_manager.primary,
                                style=theme_manager.primary_button_style,
                                on_click=confirm_disable_clicked,
                            ),
                        ],
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    ),
                ],
                spacing=theme_manager.space_12,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            ),
            width=theme_manager.modal_width,
            height=theme_manager.modal_height,
            padding=theme_manager.modal_padding,
            bgcolor=theme_manager.card_bg,
            border_radius=theme_manager.radius_lg,
            border=theme_manager.card_border,
            shadow=theme_manager.card_shadow,
            alignment=ft.Alignment.CENTER,
            on_click=lambda _: None,
        )
        disable_password_overlay.bgcolor = theme_manager.modal_overlay_bg
        disable_password_overlay.blur = ft.Blur(theme_manager.modal_overlay_blur, theme_manager.modal_overlay_blur)
        disable_password_overlay.expand = True
        disable_password_overlay.alignment = ft.Alignment.CENTER

        return ft.Stack(
            [
                main_content,
                disable_password_overlay,
            ],
            expand=True,
        )

    root_container = ft.Container(
        content=build_ui(snapshot_state()),
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

    return root_container


__all__ = ["settings_modal"]
