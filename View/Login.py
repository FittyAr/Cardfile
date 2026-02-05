import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt
from config.config import Config
from theme.manager import ThemeManager

theme_manager = ThemeManager()

async def login_view(page: ft.Page):
    config = Config()
    
    async def change_language(e):
        selected_lang = language_dd.value
        if config.set_language(selected_lang):
            # Actualizar todos los textos en la vista
            username.label = config.get_text("login.username.label")
            username.hint_text = config.get_text("login.username.hint")
            password.label = config.get_text("login.password.label")
            btn_login.text = config.get_text("login.buttons.login")
            btn_exit.text = config.get_text("login.buttons.exit")
            register_link.text = config.get_text("login.register_link")
            title_text.value = config.get_text("login.title")
            page.update()

    # Dropdown para selección de idioma
    language_options = [
        ft.DropdownOption(opt["value"], opt["text"])
        for opt in config.get_language_options()
    ]
    
    language_dd = ft.Dropdown(
        width=120,
        options=language_options,
        value=config.current_language,
        on_select=change_language,
    )

    title_text = ft.Text(
        config.get_text("login.title"),
        size=28,
        weight=ft.FontWeight.BOLD
    )

    async def login_clicked(e):
        if not username.value or not password.value:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("login.errors.empty_fields")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
            return
        
        session = get_session()
        try:
            usuario = session.query(Usuario).filter(
                Usuario.email == username.value.strip(),
                Usuario.is_active == True
            ).first()
            
            if usuario and verify_password(usuario.contraseña, password.value):
                usuario.last_login = datetime.now()
                session.commit()
                # Convertir a string para evitar errores de tipo en shared_preferences
                await page.shared_preferences.set("user_id", str(usuario.id))
                await page.shared_preferences.set("user_name", usuario.nombre)
                await page.push_route("/Card")
            else:
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(config.get_text("login.errors.invalid_credentials")),
                    bgcolor=ft.Colors.RED_400,
                    action="Ok"
                ))
                password.value = ""
                page.update()

        except Exception as e:
            session.rollback()
            print(f"Error de login: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("login.errors.login_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
        finally:
            session.close()

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
        border_color=ft.Colors.OUTLINE,
        focused_border_color=ft.Colors.BLUE_400,
        width=320,
        text_size=14,
        on_submit=login_clicked,
    )
    
    password = ft.TextField(
        label=config.get_text("login.password.label"),
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_OUTLINE,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=ft.Colors.BLUE_400,
        width=320,
        text_size=14,
        on_submit=login_clicked,
    )
 
    btn_login = ft.ElevatedButton(
        content=ft.Text(config.get_text("login.buttons.login"), weight=ft.FontWeight.BOLD),
        width=150,
        height=45,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=10)),
        on_click=login_clicked
    )
 
    btn_exit = ft.TextButton(
        content=ft.Text(config.get_text("login.buttons.exit"), color=ft.Colors.RED_400),
        on_click=exit_clicked
    )
 
    async def go_to_register(e):
        await page.push_route("/newUser")

    register_link = ft.TextButton(
        content=ft.Text(config.get_text("login.register_link"), color=ft.Colors.BLUE_400),
        on_click=go_to_register
    )

    return ft.Container(
        content=ft.Column(
            [
                # Header con Selector de Idioma
                ft.Row(
                    [
                        ft.Text("CardFile", size=20, weight=ft.FontWeight.BOLD, color=ft.Colors.BLUE_400),
                        language_dd
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Icon(ft.Icons.PERSON_PIN_ROUNDED, size=64, color=ft.Colors.BLUE_400),
                            ft.Text(config.get_text("login.title"), size=28, weight=ft.FontWeight.BOLD),
                            ft.Text("Ingresa tus credenciales para continuar", size=14, color=ft.Colors.with_opacity(0.6, ft.Colors.ON_SURFACE)),
                        ],
                        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                        spacing=10,
                    ),
                    padding=ft.Padding.symmetric(vertical=20),
                ),
                
                ft.Column(
                    [
                        username,
                        password,
                    ],
                    spacing=15,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                
                ft.Container(height=10),
                
                btn_login,
                
                ft.Row(
                    [
                        register_link,
                        btn_exit,
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
            ],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=10,
        ),
        width=400,
        padding=40,
        bgcolor=ft.Colors.SURFACE,
        border_radius=20,
        border=ft.border.all(1, ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
        shadow=ft.BoxShadow(
            blur_radius=30,
            color=ft.Colors.with_opacity(0.1, ft.Colors.BLACK),
            offset=ft.Offset(0, 10),
        ),
        alignment=ft.Alignment.CENTER,
    )

__all__ = ['login_view']
