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
        prefix_icon=ft.Icons.LABEL_OUTLINE,
        border_color=ft.Colors.OUTLINE,
        focused_border_color=ft.Colors.BLUE_400,
        width=320,
        on_submit=save_clicked,
        autofocus=True,
        text_size=14,
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("new_card.buttons.save"), weight=ft.FontWeight.BOLD),
        width=150,
        height=45,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE_400,
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
                        ft.Icon(ft.Icons.ADD_CIRCLE_OUTLINE, color=ft.Colors.BLUE_400),
                        ft.Text(config.get_text("new_card.title"), size=20, weight=ft.FontWeight.BOLD),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=10,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                ft.Container(height=10),
                
                ft.Text(
                    "Ingresa el título para tu nueva tarjeta de notas.",
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
    )

# Exportamos la función
__all__ = ['new_card_view'] 
