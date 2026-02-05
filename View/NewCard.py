import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config

async def new_card_view(page: ft.Page):
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
            
            await page.push_route("/Card")
            
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
        await page.push_route("/Card")

    # Campo para el nombre de la tarjeta
    card_name = ft.TextField(
        label=config.get_text("new_card.name.label"),
        border_color=ft.Colors.BLUE,
        width=300,
        on_submit=save_clicked,
        text_align=ft.TextAlign.LEFT,
        autofocus=True
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("new_card.buttons.save")),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        content=ft.Text(config.get_text("new_card.buttons.cancel")),
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
                    config.get_text("new_card.title"),
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

# Exportamos la función
__all__ = ['new_card_view'] 
