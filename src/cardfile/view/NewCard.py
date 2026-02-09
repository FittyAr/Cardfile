import flet as ft
from cardfile.data.database.connection import get_session
from cardfile.data.models.ficha import Ficha
from datetime import datetime
from cardfile.config.config import Config
from typing import Callable
from cardfile.theme.manager import ThemeManager

theme_manager = ThemeManager()

async def new_card_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    # Siempre crear una nueva instancia de Config
    config = Config()
    
    async def save_clicked(e):
        from cardfile.view.components.auth_manager import AuthManager
        auth_manager = AuthManager(page)
        
        session = get_session()
        try:
            # Obtener el ID del usuario actual (real o Guest)
            user_id = await auth_manager.get_authenticated_user_id()
            
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
            prefs = ft.SharedPreferences()
            await prefs.set("selected_ficha", ficha_data)
            
            await on_success()
            
        except Exception as e:
            session.rollback()
            print(f"Error al guardar ficha: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("new_card.errors.save_error")),
                bgcolor=ft.Colors.RED_400,
                action=config.get_text("common.buttons.ok"),
                duration=2000
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
        border_color=theme_manager.border_color,
        focused_border_color=theme_manager.primary,
        width=theme_manager.input_width,
        on_submit=save_clicked,
        autofocus=True,
        text_size=theme_manager.text_size_md,
        color=theme_manager.text,
        label_style=theme_manager.text_style_label,
    )

    # Botones
    btn_save = ft.Button(
        content=ft.Text(config.get_text("new_card.buttons.save"), weight=ft.FontWeight.BOLD),
        width=theme_manager.button_width,
        height=theme_manager.button_height,
        color=ft.Colors.WHITE,
        bgcolor=theme_manager.primary,
        style=theme_manager.primary_button_style,
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
                        ft.Icon(ft.Icons.ADD_CIRCLE_OUTLINE, color=theme_manager.primary, size=theme_manager.icon_size_lg),
                        ft.Text(config.get_text("new_card.title"), size=theme_manager.text_size_xxl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=theme_manager.space_12,
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                
                ft.Container(height=theme_manager.space_12),
                
                ft.Text(
                    config.get_text("new_card.subtitle"),
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
__all__ = ['new_card_modal'] 
