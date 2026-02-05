import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config

async def edit_card_view(page: ft.Page):
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
                
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(config.get_text("edit_card.messages.success")),
                    bgcolor=ft.Colors.GREEN_400,
                    action="Ok"
                ))
                page.update()
                page.go("/Card")
            
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
        page.go("/Card")

    # Campo para el nombre de la tarjeta
    card_name = ft.TextField(
        label=config.get_text("edit_card.name.label"),
        border_color=ft.Colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
        on_submit=save_clicked,
        autofocus=True,
        value=selected_ficha["title"] if selected_ficha else ""
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("edit_card.buttons.update")),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        content=ft.Text(config.get_text("edit_card.buttons.cancel")),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.RED,
        on_click=cancel_clicked
    )

    # Contenedor principal
    return ft.Container(
        width=400,
        height=300,
        border=ft.border.all(2, ft.Colors.BLUE_200),
        border_radius=15,
        padding=ft.Padding.all(30),
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                ft.Text(
                    config.get_text("edit_card.title"),
                    size=24,
                    weight=ft.FontWeight.BOLD
                ),
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                card_name,
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                ft.Row(
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    controls=[btn_cancel, btn_save],
                ),
            ],
        ),
        alignment=ft.Alignment.CENTER,
    )

__all__ = ['edit_card_view'] 
