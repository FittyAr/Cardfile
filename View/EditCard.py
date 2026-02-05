import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config
from typing import Callable

async def edit_card_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    # Inicializar Config
    config = Config()
    
    selected_ficha_json = await page.shared_preferences.get("selected_ficha")
    if not selected_ficha_json:
        return ft.Text("Error: No hay ficha seleccionada")
    
    # Parsear el JSON
    import json
    selected_ficha = json.loads(selected_ficha_json)

    async def save_clicked(e):
        if not card_name.value:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("edit_card.name.empty_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
            return
        
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == selected_ficha["id"]).first()
            if ficha:
                ficha.title = card_name.value.strip()
                ficha.updated_at = datetime.now()
                session.commit()
                
                # Actualizar shared_preferences
                import json
                ficha_data = json.dumps({
                    "id": ficha.id,
                    "title": ficha.title,
                    "descripcion": ficha.descripcion
                })
                await page.shared_preferences.set("selected_ficha", ficha_data)
                
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(config.get_text("edit_card.messages.success")),
                    bgcolor=ft.Colors.GREEN_400,
                    action="Ok"
                ))
                page.update()
                await on_success()
            
        except Exception as e:
            session.rollback()
            print(f"Error al actualizar ficha: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("edit_card.messages.error")),
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
        label=config.get_text("edit_card.name.label"),
        prefix_icon=ft.Icons.EDIT_NOTE_ROUNDED,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=ft.Colors.BLUE_400,
        width=320,
        on_submit=save_clicked,
        autofocus=True,
        text_size=14,
        value=selected_ficha["title"] if selected_ficha else ""
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("edit_card.buttons.update"), weight=ft.FontWeight.BOLD),
        width=150,
        height=45,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE_400,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=10)),
        on_click=save_clicked
    )

    btn_cancel = ft.TextButton(
        content=ft.Text(config.get_text("edit_card.buttons.cancel"), color=ft.Colors.RED_400),
        on_click=cancel_clicked
    )

    return ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Icon(ft.Icons.EDIT_ROUNDED, color=ft.Colors.BLUE_400),
                        ft.Text(config.get_text("edit_card.title"), size=20, weight=ft.FontWeight.BOLD),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=10,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                ft.Container(height=10),
                
                ft.Text(
                    "Actualiza el título de tu tarjeta.",
                    size=14,
                    color=ft.Colors.with_opacity(0.6, ft.Colors.ON_SURFACE),
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
        bgcolor=ft.Colors.SURFACE,
        border_radius=20,
        border=ft.border.all(1, ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
        shadow=ft.BoxShadow(
            blur_radius=30,
            color=ft.Colors.with_opacity(0.1, ft.Colors.BLACK),
            offset=ft.Offset(0, 10),
        ),
        alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )

# Exportamos la función
__all__ = ['edit_card_modal']
