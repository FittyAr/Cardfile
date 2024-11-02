import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt

def login_view(page: ft.Page):
    # Campos de texto
    username = ft.TextField(
        label="Usuario",
        border_color=ft.colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
    )
    
    password = ft.TextField(
        label="Contraseña",
        password=True,
        can_reveal_password=True,
        border_color=ft.colors.BLUE,
        width=300,
    )

    def verify_password(stored_hash: str, provided_password: str) -> bool:
        """Verifica si la contraseña proporcionada coincide con el hash almacenado."""
        try:
            return bcrypt.checkpw(
                provided_password.encode('utf-8'),
                stored_hash.encode('utf-8')
            )
        except Exception:
            return False

    def login_clicked(e):
        if not username.value or not password.value:
            snack = ft.SnackBar(content=ft.Text("Por favor complete todos los campos"))
            page.show_snack_bar = snack
            page.update()
            return
        
        session = get_session()
        try:
            # Mejorar la consulta para ser más específica
            usuario = session.query(Usuario).filter(
                Usuario.email == username.value.strip(),
                Usuario.is_active == True
            ).first()
            
            if usuario and verify_password(usuario.contraseña, password.value):
                usuario.last_login = datetime.now()
                session.commit()
                
                # Almacenar datos de sesión
                page.client_storage.set("user_id", usuario.id)
                page.client_storage.set("user_name", usuario.nombre)
                
                page.go("/Main")
            else:
                snack = ft.SnackBar(content=ft.Text("Usuario o contraseña incorrectos"))
                page.show_snack_bar = snack
                page.update()

        except Exception as e:
            session.rollback()
            print(f"Error de login: {str(e)}")
            snack = ft.SnackBar(content=ft.Text("Error al intentar iniciar sesión"))
            page.show_snack_bar = snack
            page.update()
        finally:
            session.close()

    def exit_clicked(e=None):
        page.window_destroy()

    # Botones
    btn_login = ft.ElevatedButton(
        text="Ingresar",
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.BLUE,
        on_click=login_clicked
    )

    btn_exit = ft.ElevatedButton(
        text="Salir",
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.RED,
        on_click=exit_clicked
    )

    # Contenedor principal
    return ft.Container(
        width=400,
        height=500,
        bgcolor=ft.colors.WHITE10,
        border=ft.border.all(2, ft.colors.BLUE_200),
        border_radius=15,
        padding=30,
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                ft.Icon(ft.icons.PERSON_OUTLINE, size=50, color=ft.colors.BLUE),
                ft.Text("Iniciar Sesión", size=28, weight=ft.FontWeight.BOLD),
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                
                # Contenedor para los campos de entrada
                ft.Container(
                    content=ft.Column(
                        spacing=15,
                        controls=[
                            username,
                            password,
                        ],
                    ),
                ),
                
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                
                # Contenedor para los botones
                ft.Container(
                    content=ft.Row(
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                        controls=[btn_exit, btn_login],
                    ),
                ),
                
                ft.TextButton(
                    text="¿No tienes cuenta? Regístrate aquí",
                    on_click=lambda _: page.go("/newUser")
                ),
            ],
        ),
        alignment=ft.alignment.center,
    )

# Exportamos la función
__all__ = ['login_view']