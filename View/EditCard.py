import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config
from typing import Callable
from theme.manager import ThemeManager

theme_manager = ThemeManager()

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
        
        from View.components.auth_manager import AuthManager
        auth_manager = AuthManager(page)
        
        session = get_session()
        try:
            user_id = await auth_manager.get_authenticated_user_id()
            ficha = session.query(Ficha).filter(
                Ficha.id == selected_ficha["id"],
                Ficha.usuario_id == user_id
            ).first()
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
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        on_submit=save_clicked,
        autofocus=True,
        text_size=theme_manager.text_size_md,
        value=selected_ficha["title"] if selected_ficha else "",
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )

    # Botones
    btn_save = ft.ElevatedButton(
        content=ft.Text(config.get_text("edit_card.buttons.update"), weight=ft.FontWeight.BOLD),
        width=theme_manager.button_width,
        height=theme_manager.button_height,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=theme_manager.primary_button_style,
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
                        ft.Icon(ft.Icons.EDIT_ROUNDED, color=theme_manager.primary, size=theme_manager.icon_size_lg),
                        ft.Text(config.get_text("edit_card.title"), size=theme_manager.text_size_xxl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=theme_manager.space_12,
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                
                ft.Container(height=theme_manager.space_12),
                
                ft.Text(
                    "Cambia el nombre de tu tarjeta.",
                    size=theme_manager.text_size_md,
                    color=theme_manager.subtext,
                    text_align=ft.TextAlign.CENTER,
                ),
                
                ft.Container(height=theme_manager.space_12),
                
                card_name,
                
                ft.Container(height=theme_manager.space_12),
                
                ft.Container(
                    content=ft.Row(
                        [
                            btn_cancel,
                            btn_save,
                        ],
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    ),
                    padding=ft.Padding.only(top=theme_manager.space_12),
                ),
            ],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            tight=True,
            spacing=theme_manager.space_12,
        ),
        width=theme_manager.modal_width,
        height=theme_manager.modal_height,
        padding=theme_manager.modal_padding,
        bgcolor=theme_manager.card_bg,
        border_radius=theme_manager.radius_lg,
        border=theme_manager.card_border,
        shadow=theme_manager.card_shadow,
        alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )

# Exportamos la función
__all__ = ['edit_card_modal']
