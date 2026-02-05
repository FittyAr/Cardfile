import flet as ft
from data.database.connection import get_session
from data.models.usuario import Usuario
from datetime import datetime
import bcrypt
import re
import json
import os
import asyncio
from config.config import Config
from theme.manager import ThemeManager

theme_manager = ThemeManager()

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

    # Campos de texto con diseño moderno
    nombre = ft.TextField(
        label=t['fields']['name']['label'],
        prefix_icon=ft.Icons.PERSON_OUTLINE,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=theme_manager.primary,
        width=320,
        text_size=14,
    )
    
    email = ft.TextField(
        label=t['fields']['email']['label'],
        prefix_icon=ft.Icons.EMAIL_OUTLINED,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=theme_manager.primary,
        width=320,
        text_size=14,
    )
    
    password = ft.TextField(
        label=t['fields']['password']['label'],
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_OUTLINE,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=theme_manager.primary,
        width=320,
        text_size=14,
    )
    
    confirm_password = ft.TextField(
        label=t['fields']['confirm_password']['label'],
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_RESET_ROUNDED,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=theme_manager.primary,
        width=320,
        text_size=14,
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['empty_fields']), bgcolor=ft.Colors.RED_400))
            page.update()
            return
            
        if password.value != confirm_password.value:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['passwords_dont_match']), bgcolor=ft.Colors.RED_400))
            page.update()
            return
        
        # Validar longitud mínima de contraseña
        if len(password.value) < 8:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['password_length']), bgcolor=ft.Colors.RED_400))
            page.update()
            return
        
        if not is_valid_email(email.value):
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['invalid_email']), bgcolor=ft.Colors.RED_400))
            page.update()
            return
        
        session = get_session()
        try:
            # Verificar si el email ya existe
            existing_user = session.query(Usuario).filter(
                Usuario.email == email.value
            ).first()
            
            if existing_user:
                page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['email_exists']), bgcolor=ft.Colors.RED_400))
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['success']['user_created']), bgcolor=ft.Colors.GREEN_400))
            page.update()
            
            # Limpiar campos
            nombre.value = ""
            email.value = ""
            password.value = ""
            confirm_password.value = ""
            page.update()
            
            # Redirigir al login
            await page.push_route("/Login")
            
        except Exception as e:
            session.rollback()
            print(f"Error al crear usuario: {str(e)}")
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['create_error']), bgcolor=ft.Colors.RED_400))
            page.update()
        finally:
            session.close()

    async def cancel_clicked(e):
        await page.push_route("/Login")

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(t['buttons']['save'], weight=ft.FontWeight.BOLD),
        width=150,
        height=45,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=10)),
        on_click=save_clicked
    )

    btn_cancel = ft.TextButton(
        content=ft.Text(t['buttons']['cancel'], color=ft.Colors.RED_400),
        on_click=cancel_clicked
    )

    # Contenedor principal con diseño moderno
    return ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Text("CardFile", size=20, weight=ft.FontWeight.BOLD, color=ft.Colors.BLUE_400),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Icon(ft.Icons.PERSON_ADD_ROUNDED, size=64, color=ft.Colors.BLUE_400),
                            ft.Text(t['title'], size=28, weight=ft.FontWeight.BOLD),
                            ft.Text("Crea una cuenta para empezar", size=14, color=ft.Colors.with_opacity(0.6, ft.Colors.ON_SURFACE)),
                        ],
                        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                        spacing=10,
                    ),
                    padding=ft.Padding.symmetric(vertical=10),
                ),
                
                ft.Column(
                    [
                        nombre,
                        email,
                        password,
                        confirm_password,
                    ],
                    spacing=15,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                
                ft.Container(height=10),
                
                btn_save,
                
                ft.Row(
                    [
                        ft.TextButton(
                            content=ft.Text(t['login_link'], color=ft.Colors.BLUE_400),
                            on_click=lambda _: asyncio.create_task(page.push_route("/Login"))
                        ),
                        btn_cancel,
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
