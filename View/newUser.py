import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt

def newUser_view(page: ft.Page):
    # Campos de texto
    nombre = ft.TextField(
        label="Nombre",
        border_color=ft.colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
    )
    
    email = ft.TextField(
        label="Email",
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
    
    confirm_password = ft.TextField(
        label="Confirmar Contraseña",
        password=True,
        can_reveal_password=True,
        border_color=ft.colors.BLUE,
        width=300,
    )

    def hash_password(password: str) -> str:
        """Genera un hash seguro para la contraseña."""
        # Generar salt y hash
        salt = bcrypt.gensalt()
        hashed = bcrypt.hashpw(password.encode('utf-8'), salt)
        return hashed.decode('utf-8')  # Convertir bytes a string para almacenar

    def save_clicked(e):
        # Validaciones
        if not all([nombre.value, email.value, password.value, confirm_password.value]):
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Por favor complete todos los campos"))
            )
            return
            
        if password.value != confirm_password.value:
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Las contraseñas no coinciden"))
            )
            return
        
        # Validar longitud mínima de contraseña
        if len(password.value) < 8:
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("La contraseña debe tener al menos 8 caracteres"))
            )
            return
        
        session = get_session()
        try:
            # Verificar si el email ya existe
            existing_user = session.query(Usuario).filter(
                Usuario.email == email.value
            ).first()
            
            if existing_user:
                page.show_snack_bar(
                    ft.SnackBar(content=ft.Text("El email ya está registrado"))
                )
                return
            
            # Crear nuevo usuario con contraseña hasheada
            hashed_password = hash_password(password.value)
            
            nuevo_usuario = Usuario(
                nombre=nombre.value,
                email=email.value,
                contraseña=hashed_password,
                is_active=True,
                created_at=datetime.now(),
                updated_at=datetime.now()
            )
            
            session.add(nuevo_usuario)
            session.commit()
            
            # Mostrar mensaje de éxito
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Usuario creado exitosamente"))
            )
            
            # Limpiar campos
            nombre.value = ""
            email.value = ""
            password.value = ""
            confirm_password.value = ""
            page.update()
            
            # Opcional: redirigir al login
            page.go("/Login2")
            
        except Exception as e:
            session.rollback()
            print(f"Error al crear usuario: {str(e)}")
            page.show_snack_bar(
                ft.SnackBar(content=ft.Text("Error al crear el usuario"))
            )
        finally:
            session.close()

    def cancel_clicked(e):
        page.go("/Login2")

    # Botones
    btn_save = ft.ElevatedButton(
        text="Guardar",
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        text="Cancelar",
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.RED,
        on_click=cancel_clicked
    )

    # Contenedor principal
    return ft.Container(
        width=400,
        height=500,
        bgcolor=ft.colors.WHITE,
        border_radius=10,
        padding=20,
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            controls=[
                ft.Text("Nuevo Usuario", size=24, weight=ft.FontWeight.BOLD),
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                nombre,
                ft.Divider(height=10, color=ft.colors.TRANSPARENT),
                email,
                ft.Divider(height=10, color=ft.colors.TRANSPARENT),
                password,
                ft.Divider(height=10, color=ft.colors.TRANSPARENT),
                confirm_password,
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                ft.Row(
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    controls=[
                        btn_cancel,
                        btn_save,
                    ],
                ),
            ],
        ),
    ) 