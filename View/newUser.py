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
    config = Config()
    # Aplicar modo oscuro/claro según el tema
    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
    
    from View.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)
    if await auth_manager.is_authenticated():
        await page.push_route("/Card")
        return ft.Container()

    # Cargar traducciones usando el idioma configurado
    translations = load_translations(config.current_language)
    t = translations['new_user']

    # Campos de texto con diseño moderno
    nombre = ft.TextField(
        label=t['fields']['name']['label'],
        prefix_icon=ft.Icons.PERSON_OUTLINE,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )
    
    email = ft.TextField(
        label=t['fields']['email']['label'],
        prefix_icon=ft.Icons.EMAIL_OUTLINED,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )
    
    password = ft.TextField(
        label=t['fields']['password']['label'],
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_OUTLINE,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )
    
    confirm_password = ft.TextField(
        label=t['fields']['confirm_password']['label'],
        password=True,
        can_reveal_password=True,
        prefix_icon=ft.Icons.LOCK_RESET_ROUNDED,
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['empty_fields']), bgcolor=ft.Colors.RED_400, duration=2000))
            page.update()
            return
            
        if password.value != confirm_password.value:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['passwords_dont_match']), bgcolor=ft.Colors.RED_400, duration=2000))
            page.update()
            return
        
        # Validar longitud mínima de contraseña
        if len(password.value) < 8:
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['password_length']), bgcolor=ft.Colors.RED_400, duration=2000))
            page.update()
            return
        
        if not is_valid_email(email.value):
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['invalid_email']), bgcolor=ft.Colors.RED_400, duration=2000))
            page.update()
            return
        
        session = get_session()
        try:
            # Verificar si el email ya existe
            existing_user = session.query(Usuario).filter(
                Usuario.email == email.value
            ).first()
            
            if existing_user:
                page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['email_exists']), bgcolor=ft.Colors.RED_400, duration=2000))
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['success']['user_created']), bgcolor=ft.Colors.GREEN_400, duration=2000))
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
            page.show_dialog(ft.SnackBar(content=ft.Text(t['errors']['create_error']), bgcolor=ft.Colors.RED_400, duration=2000))
            page.update()
        finally:
            session.close()

    async def cancel_clicked(e):
        await page.push_route("/Login")

    # Botones
    btn_save = ft.Button(
        content=ft.Text(t['buttons']['save'], weight=ft.FontWeight.BOLD),
        width=theme_manager.button_width_lg,
        height=theme_manager.button_height_lg,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=theme_manager.primary_button_style,
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
                        ft.Text("CardFile", size=theme_manager.text_size_xl, weight=ft.FontWeight.BOLD, color=theme_manager.primary),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Icon(ft.Icons.PERSON_ADD_ROUNDED, size=theme_manager.icon_size_xl, color=theme_manager.primary),
                            ft.Text(t['title'], size=theme_manager.text_size_3xl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                            ft.Text("Crea una cuenta para empezar", size=theme_manager.text_size_md, color=theme_manager.subtext),
                        ],
                        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                        spacing=theme_manager.space_12,
                    ),
                    padding=ft.Padding.symmetric(vertical=theme_manager.space_12),
                ),
                
                ft.Column(
                    [
                        nombre,
                        email,
                        password,
                        confirm_password,
                    ],
                    spacing=theme_manager.space_16,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                
                ft.Container(height=theme_manager.space_12),
                
                btn_save,
                
                ft.Row(
                    [
                        ft.TextButton(
                            content=ft.Text(t['login_link'], color=theme_manager.primary),
                            on_click=lambda _: asyncio.create_task(page.push_route("/Login"))
                        ),
                        btn_cancel,
                    ],
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
