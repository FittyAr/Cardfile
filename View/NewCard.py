import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config

def new_card_view(page: ft.Page):
    # Siempre crear una nueva instancia de Config
    config = Config()

    def save_clicked(e):
        if not card_name.value:
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text(config.get_text("new_card.name.empty_error")),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
            return
        
        session = get_session()
        try:
            user_id = page.client_storage.get("user_id")
            
            nueva_ficha = Ficha(
                title=card_name.value.strip(),
                descripcion="",
                usuario_id=user_id,
                created_at=datetime.now(),
                updated_at=datetime.now()
            )
            
            session.add(nueva_ficha)
            session.commit()
            
            page.go("/Card")
            
        except Exception as e:
            session.rollback()
            print(f"Error al guardar ficha: {str(e)}")
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text(config.get_text("new_card.errors.save_error")),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
        finally:
            session.close()

    def cancel_clicked(e):
        page.go("/Card")

    # Campo para el nombre de la tarjeta
    card_name = ft.TextField(
        label=config.get_text("new_card.name.label"),
        border_color=ft.colors.BLUE,
        width=300,
        on_submit=save_clicked,
        text_align=ft.TextAlign.LEFT,
        autofocus=True
    )

    # Botones
    btn_save = ft.ElevatedButton(
        text=config.get_text("new_card.buttons.save"),
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        text=config.get_text("new_card.buttons.cancel"),
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.RED,
        on_click=cancel_clicked
    )

    # Contenedor principal
    return ft.Container(
        width=400,
        height=300,
        border=ft.border.all(2, ft.colors.BLUE_200),
        border_radius=15,
        padding=30,
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                ft.Text(
                    config.get_text("new_card.title"),
                    size=24,
                    weight=ft.FontWeight.BOLD
                ),
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                card_name,
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                ft.Row(
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    controls=[btn_cancel, btn_save],
                ),
            ],
        ),
        alignment=ft.alignment.center,
    )

# Exportamos la función
__all__ = ['new_card_view'] 