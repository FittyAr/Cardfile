import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt

def login2_view(page: ft.Page):
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
        if username.value == "" or password.value == "":
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Por favor complete todos los campos"))
            )
            return
        
        session = get_session()
        try:
            # Primero buscar usuario solo por email
            usuario = session.query(Usuario).filter(
                Usuario.email == username.value,
                Usuario.is_active == True
            ).first()
            
            # Verificar la contraseña
            if usuario and verify_password(usuario.contraseña, password.value):
                usuario.last_login = datetime.now()
                session.commit()
                
                page.session.set("user_id", usuario.id)
                page.session.set("user_name", usuario.nombre)
                
                page.go("/Main")
            else:
                page.show_snack_bar(
                    ft.SnackBar(content=ft.Text("Usuario o contraseña incorrectos"))
                )
                
        except Exception as e:
            session.rollback()
            print(f"Error de login: {str(e)}")
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Error al intentar iniciar sesión"))
            )
        finally:
            session.close()

    def exit_clicked(e):
        import sys
        import platform
        
        if platform.system() == "Windows":
            sys.exit(0)
        else:
            page.go("/")

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
        height=300,
        #bgcolor=ft.colors.WHITE,
        border_radius=10,
        padding=20,
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            controls=[
                ft.Text("Iniciar Sesión", size=24, weight=ft.FontWeight.BOLD),
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                username,
                ft.Divider(height=10, color=ft.colors.TRANSPARENT),
                password,
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                ft.Row(
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    controls=[
                        btn_exit,
                        btn_login,
                    ],
                ),
            ],
        ),
    )

# Exportamos la función
__all__ = ['login2_view']