import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt
from config.config import Config
from config.runtime import is_web_runtime
from theme.manager import ThemeManager

theme_manager = ThemeManager()

async def login_view(page: ft.Page):
    config = Config()
    is_web = is_web_runtime(page)
    
    # Aplicar modo oscuro/claro según el tema
    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
    
    async def change_language(e):
        selected_lang = language_dd.value
        if config.set_language(selected_lang):
            # Actualizar todos los textos en la vista
            username.label = config.get_text("login.username.label")
            username.hint_text = config.get_text("login.username.hint")
            password.label = config.get_text("login.password.label")
            btn_login.content.value = config.get_text("login.buttons.login")
            if btn_exit:
                btn_exit.content.value = config.get_text("login.buttons.exit")
            register_link.content.value = config.get_text("login.register_link")
            title_text.value = config.get_text("login.title")
            page.update()

    # Dropdown para selección de idioma
    language_options = [
        ft.DropdownOption(opt["value"], opt["text"])
        for opt in config.get_language_options()
    ]
    
    language_dd = ft.Dropdown(
        width=theme_manager.language_dropdown_width,
        options=language_options,
        value=config.current_language,
        on_select=change_language,
    )

    title_text = ft.Text(
        config.get_text("login.title"),
        size=theme_manager.text_size_3xl,
        weight=ft.FontWeight.BOLD,
        color=theme_manager.text
    )

    from View.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)
    if await auth_manager.is_authenticated():
        await page.push_route("/Card")
        return ft.Container()
    
    async def login_clicked(e):
        if not username.value or not password.value:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("login.errors.empty_fields")),
                bgcolor=ft.Colors.RED_400,
                action="Ok",
                duration=2000
            ))
            page.update()
            return

        try:
            # Usar AuthManager para el login y la persistencia
            if await auth_manager.login(username.value.strip(), password.value):
                await page.push_route("/Card")
            else:
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(config.get_text("login.errors.invalid_credentials")),
                    bgcolor=ft.Colors.RED_400,
                    action="Ok",
                    duration=2000
                ))
                password.value = ""
                page.update()

        except Exception as e:
            print(f"Error de login: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("login.errors.login_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok",
                duration=2000
            ))
            page.update()
    def verify_password(stored_hash: str, provided_password: str) -> bool:
        """Verifica si la contraseña proporcionada coincide con el hash almacenado."""
        try:
            return bcrypt.checkpw(
                provided_password.encode('utf-8'),
                stored_hash.encode('utf-8')
            )
        except Exception:
            return False

    async def exit_clicked(e=None):
        # En modo web con Browser, no hay window.destroy()
        # En su lugar, podemos cerrar la página o redirigir
        try:
            if hasattr(page, 'window') and page.window:
                page.window.destroy()
        except Exception:
            # En modo web, simplemente salir de la aplicación
            pass

    username = ft.TextField(
        label=config.get_text("login.username.label"),
        hint_text=config.get_text("login.username.hint"),
        prefix_icon=ft.Icons.PERSON_OUTLINE,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        on_submit=login_clicked,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )
    
    password = ft.TextField(
        label=config.get_text("login.password.label"),
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_OUTLINE,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        on_submit=login_clicked,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )
 
    btn_login = ft.Button(
        content=ft.Text(config.get_text("login.buttons.login"), weight=ft.FontWeight.BOLD),
        width=theme_manager.button_width_lg,
        height=theme_manager.button_height_lg,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=theme_manager.primary_button_style,
        on_click=login_clicked
    )
 
    btn_exit = None
    if not is_web:
        btn_exit = ft.TextButton(
            content=ft.Text(config.get_text("login.buttons.exit"), color=ft.Colors.RED_400),
            on_click=exit_clicked
        )
 
    async def go_to_register(e):
        await page.push_route("/newUser")

    register_link = ft.TextButton(
        content=ft.Text(config.get_text("login.register_link"), color=theme_manager.primary),
        on_click=go_to_register
    )

    return ft.Container(
        content=ft.Column(
            [
                # Header con Selector de Idioma
                ft.Row(
                    [
                        ft.Text("CardFile", size=theme_manager.text_size_xl, weight=ft.FontWeight.BOLD, color=theme_manager.primary),
                        language_dd
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Icon(ft.Icons.PERSON_PIN_ROUNDED, size=theme_manager.icon_size_xl, color=theme_manager.primary),
                            ft.Text(config.get_text("login.title"), size=theme_manager.text_size_3xl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                            ft.Text("Ingresa tus credenciales para continuar", size=theme_manager.text_size_md, color=theme_manager.subtext),
                        ],
                        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                        spacing=theme_manager.space_12,
                    ),
                    padding=ft.Padding.symmetric(vertical=theme_manager.space_20),
                ),
                
                ft.Column(
                    [
                        username,
                        password,
                    ],
                    spacing=theme_manager.space_16,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                
                ft.Container(height=theme_manager.space_12),
                
                btn_login,
                
                ft.Row(
                    [register_link] + ([btn_exit] if btn_exit else []),
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
            ],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=theme_manager.space_12,
        ),
        width=theme_manager.auth_card_width,
        padding=theme_manager.auth_card_padding,
        bgcolor=theme_manager.card_bg,
        border_radius=theme_manager.radius_lg,
        border=theme_manager.card_border,
        shadow=theme_manager.card_shadow,
        alignment=ft.Alignment.CENTER,
    )

__all__ = ['login_view']
