import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt
from config.config import Config

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
                page.go("/Card")
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
        border_color=ft.Colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
        on_submit=login_clicked,
        #value="test@test.test"
    )
    
    password = ft.TextField(
        label=config.get_text("login.password.label"),
        password=True,
        can_reveal_password=True,
        border_color=ft.Colors.BLUE,
        width=300,
        on_submit=login_clicked,
        #value="abc123*-"
    )
 
    btn_login = ft.ElevatedButton(
        content=ft.Text(config.get_text("login.buttons.login")),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE,
        on_click=login_clicked
    )
 
    btn_exit = ft.ElevatedButton(
        content=ft.Text(config.get_text("login.buttons.exit")),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.RED,
        on_click=exit_clicked
    )
 
    register_link = ft.TextButton(
        content=ft.Text(config.get_text("login.register_link")),
        on_click=lambda _: page.go("/newUser")
    )

    # Modificar el return para incluir el selector de idioma
    return ft.Container(
        width=400,
        height=500,
        bgcolor=ft.Colors.WHITE10,
        border=ft.border.all(2, ft.Colors.BLUE_200),
        border_radius=15,
        padding=30,
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                # Agregar el selector de idioma en la parte superior
                ft.Container(
                    content=ft.Row(
                        alignment=ft.MainAxisAlignment.END,
                        controls=[language_dd],
                    ),
                    padding=ft.Padding.only(bottom=10),
                ),
                ft.Icon(ft.Icons.PERSON_OUTLINE, size=50, color=ft.Colors.BLUE),
                title_text,
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                ft.Container(
                    content=ft.Column(
                        spacing=15,
                        controls=[
                            username,
                            password,
                        ],
                    ),
                ),
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                ft.Container(
                    content=ft.Row(
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                        controls=[btn_exit, btn_login],
                    ),
                ),
                register_link,
            ],
        ),
        alignment=ft.Alignment.CENTER,
    )

__all__ = ['login_view']
