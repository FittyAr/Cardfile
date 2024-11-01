import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from sqlalchemy import select
from datetime import datetime

class Login2(ft.UserControl):
    def __init__(self, page):
        super().__init__()
        self.page = page

    def build(self):
        # Campos de texto
        self.username = ft.TextField(
            label="Usuario",
            border_color=ft.colors.BLUE,
            width=300,
            text_align=ft.TextAlign.LEFT,
        )
        
        self.password = ft.TextField(
            label="Contraseña",
            password=True,
            can_reveal_password=True,
            border_color=ft.colors.BLUE,
            width=300,
        )

        # Botones
        self.btn_login = ft.ElevatedButton(
            text="Ingresar",
            width=140,
            color=ft.colors.WHITE,
            bgcolor=ft.colors.BLUE,
            on_click=self.login_clicked
        )

        self.btn_exit = ft.ElevatedButton(
            text="Salir",
            width=140,
            color=ft.colors.WHITE,
            bgcolor=ft.colors.RED,
            on_click=self.exit_clicked
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
                    self.username,
                    ft.Divider(height=10, color=ft.colors.TRANSPARENT),
                    self.password,
                    ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                    ft.Row(
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                        controls=[
                            self.btn_exit,
                            self.btn_login,
                        ],
                    ),
                ],
            ),
        )

    def login_clicked(self, e):
        if self.username.value == "" or self.password.value == "":
            self.page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Por favor complete todos los campos"))
            )
            return
        
        session = get_session()
        try:
            # Usando SQLAlchemy ORM para la consulta
            usuario = session.query(Usuario).filter(
                Usuario.email == self.username.value,
                Usuario.contraseña == self.password.value,  # Recordar implementar hash
                Usuario.is_active == True
            ).first()
            
            if usuario:
                # Actualizar último login usando SQLAlchemy
                usuario.last_login = datetime.now()
                session.commit()
                
                # Guardar información del usuario en la sesión
                self.page.session.set("user_id", usuario.id)
                self.page.session.set("user_name", usuario.nombre)
                
                self.page.go("/Main")
            else:
                self.page.show_snack_bar(
                    ft.SnackBar(content=ft.Text("Usuario o contraseña incorrectos"))
                )
                
        except Exception as e:
            session.rollback()
            print(f"Error de login: {str(e)}")
            self.page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Error al intentar iniciar sesión"))
            )
        finally:
            session.close()

    def exit_clicked(self, e):
        import sys
        import platform
        
        if platform.system() == "Windows":
            sys.exit(0)
        else:
            self.page.go("/")