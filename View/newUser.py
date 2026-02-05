import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt
import re
import json
import os
from config.config import Config

def load_translations(lang='es'):
    """Carga las traducciones del idioma especificado"""
    config = Config()
    file_path = os.path.join(config.get("app.language.path", "./lang"), f'{lang}.json')
    with open(file_path, 'r', encoding='utf-8') as file:
        return json.load(file)

async def newUser_view(page: ft.Page):
    # Obtener la configuración global
    config = Config()
    # Cargar traducciones usando el idioma configurado
    translations = load_translations(config.current_language)
    t = translations['new_user']

    # Campos de texto
    nombre = ft.TextField(
        label=t['fields']['name']['label'],
        border_color=ft.Colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
    )
    
    email = ft.TextField(
        label=t['fields']['email']['label'],
        border_color=ft.Colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
    )
    
    password = ft.TextField(
        label=t['fields']['password']['label'],
        password=True,
        can_reveal_password=True,
        border_color=ft.Colors.BLUE,
        width=300,
    )
    
    confirm_password = ft.TextField(
        label=t['fields']['confirm_password']['label'],
        password=True,
        can_reveal_password=True,
        border_color=ft.Colors.BLUE,
        width=300,
    )

    def hash_password(password: str) -> str:
        """Genera un hash seguro para la contraseña."""
        # Generar salt y hash
        salt = bcrypt.gensalt()
        hashed = bcrypt.hashpw(password.encode('utf-8'), salt)
        return hashed.decode('utf-8')  # Convertir bytes a string para almacenar

    def is_valid_email(email: str) -> bool:
        pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
        return re.match(pattern, email) is not None

    async def save_clicked(e):
        # Validaciones
        if not all([nombre.value, email.value, password.value, confirm_password.value]):
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['empty_fields'])))
            page.update()
            return
            
        if password.value != confirm_password.value:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['passwords_dont_match'])))
            page.update()
            return
        
        # Validar longitud mínima de contraseña
        if len(password.value) < 8:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['password_length'])))
            page.update()
            return
        
        if not is_valid_email(email.value):
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['invalid_email'])))
            page.update()
            return
        
        session = get_session()
        try:
            # Verificar si el email ya existe
            existing_user = session.query(Usuario).filter(
                Usuario.email == email.value
            ).first()
            
            if existing_user:
                page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['email_exists'])))
                page.update()
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['success']['user_created'])))
            page.update()
            
            # Limpiar campos
            nombre.value = ""
            email.value = ""
            password.value = ""
            confirm_password.value = ""
            page.update()
            
            # Opcional: redirigir al login
            await page.push_route("/Login")
            
        except Exception as e:
            session.rollback()
            print(f"Error al crear usuario: {str(e)}")
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['create_error'])))
            page.update()
        finally:
            session.close()

    async def cancel_clicked(e):
        await page.push_route("/Login")

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(t['buttons']['save']),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        content=ft.Text(t['buttons']['cancel']),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.RED,
        on_click=cancel_clicked
    )

    # Contenedor principal
    return ft.Container(
        width=400,
        height=600,  # Un poco más alto para acomodar los campos extra
        bgcolor=ft.Colors.WHITE10,
        border=ft.border.all(2, ft.Colors.BLUE_200),
        border_radius=15,
        padding=ft.Padding.all(30),
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                ft.Icon(ft.Icons.PERSON_ADD, size=50, color=ft.Colors.BLUE),
                ft.Text(t['title'], size=28, weight=ft.FontWeight.BOLD),
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                
                # Contenedor para los campos de entrada
                ft.Container(
                    content=ft.Column(
                        spacing=15,
                        controls=[
                            nombre,
                            email,
                            password,
                            confirm_password,
                        ],
                    ),
                ),
                
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                
                # Contenedor para los botones
                ft.Container(
                    content=ft.Row(
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                        controls=[btn_cancel, btn_save],
                    ),
                ),
                
                ft.TextButton(
                    content=ft.Text(t['login_link']),
                    on_click=lambda _: asyncio.create_task(page.push_route("/Login"))
                ),
            ],
        ),
        alignment=ft.Alignment.CENTER,
    ) 
