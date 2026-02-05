import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config
from typing import Callable
from theme.manager import ThemeManager

theme_manager = ThemeManager()

async def new_card_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    # Siempre crear una nueva instancia de Config
    config = Config()

    async def save_clicked(e):
        if not card_name.value:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("new_card.name.empty_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
            return
        
        session = get_session()
        try:
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            
            nueva_ficha = Ficha(
                title=card_name.value.strip(),
                descripcion="",
                usuario_id=user_id,
                created_at=datetime.now(),
                updated_at=datetime.now()
            )
            
            session.add(nueva_ficha)
            session.commit()
            
            # Guardar en shared_preferences para que quede seleccionada al volver
            import json
            ficha_data = json.dumps({
                "id": nueva_ficha.id,
                "title": nueva_ficha.title,
                "descripcion": nueva_ficha.descripcion
            })
            await page.shared_preferences.set("selected_ficha", ficha_data)
            
            await on_success()
            
        except Exception as e:
            session.rollback()
            print(f"Error al guardar ficha: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("new_card.errors.save_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
        finally:
            session.close()

    async def cancel_clicked(e):
        await on_close()

    # Campo para el nombre de la tarjeta
    card_name = ft.TextField(
        label=config.get_text("new_card.name.label"),
        prefix_icon=ft.Icons.LABEL_OUTLINE,
        border_color=ft.Colors.with_opacity(0.1, theme_manager.text),
        focused_border_color=theme_manager.primary,
        width=320,
        on_submit=save_clicked,
        autofocus=True,
        text_size=14,
        color=theme_manager.text,
        label_style=ft.TextStyle(color=theme_manager.subtext),
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("new_card.buttons.save"), weight=ft.FontWeight.BOLD),
        width=150,
        height=45,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=10)),
        on_click=save_clicked
    )

    btn_cancel = ft.TextButton(
        content=ft.Text(config.get_text("new_card.buttons.cancel"), color=ft.Colors.RED_400),
        on_click=cancel_clicked
    )

    return ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Icon(ft.Icons.ADD_CIRCLE_OUTLINE, color=theme_manager.primary),
                        ft.Text(config.get_text("new_card.title"), size=20, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=10,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, theme_manager.text)),
                
                ft.Container(height=10),
                
                ft.Text(
                    "Ingresa el título para tu nueva tarjeta de notas.",
                    size=14,
                    color=theme_manager.subtext,
                    text_align=ft.TextAlign.CENTER,
                ),
                
                ft.Container(height=10),
                
                card_name,
                
                ft.Container(height=20),
                
                ft.Row(
                    [
                        btn_cancel,
                        btn_save,
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
            ],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=10,
        ),
        width=400,
        padding=40,
        bgcolor=theme_manager.card_bg,
        border_radius=20,
        border=ft.border.all(1, ft.Colors.with_opacity(0.1, theme_manager.text)),
        shadow=ft.BoxShadow(
            blur_radius=30,
            color=ft.Colors.with_opacity(0.1, ft.Colors.BLACK),
            offset=ft.Offset(0, 10),
        ),
        alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )

# Exportamos la función
__all__ = ['new_card_modal'] 
