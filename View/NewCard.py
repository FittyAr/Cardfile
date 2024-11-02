import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime

def new_card_view(page: ft.Page):
    def save_clicked(e):
        if not card_name.value:
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text("Por favor ingrese un nombre para la tarjeta"),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
            return
        
        session = get_session()
        try:
            # Obtener el ID del usuario de la sesión
            user_id = page.client_storage.get("user_id")
            
            # Crear nueva ficha
            nueva_ficha = Ficha(
                title=card_name.value.strip(),
                descripcion="",  # Descripción inicial vacía
                usuario_id=user_id,
                created_at=datetime.now(),
                updated_at=datetime.now()
            )
            
            session.add(nueva_ficha)
            session.commit()
            
            page.go("/Card")  # Volver a la vista principal
            
        except Exception as e:
            session.rollback()
            print(f"Error al guardar ficha: {str(e)}")
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text("Error al guardar la ficha"),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
        finally:
            session.close()

    def cancel_clicked(e):
        page.go("/Card")  # Volver a la vista principal sin guardar

    # Campo para el nombre de la tarjeta
    card_name = ft.TextField(
        label="Nombre de la Tarjeta",
        border_color=ft.colors.BLUE,
        width=300,
        text_align=ft.TextAlign.LEFT,
    )

    # Botones
    btn_save = ft.ElevatedButton(
        text="Guardar",
        width=140,
        color=ft.colors.WHITE,
        bgcolor=ft.colors.BLUE,
        on_click=save_clicked
    )

    btn_cancel = ft.ElevatedButton(
        text="Cancelar",
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
                ft.Text("Nueva Tarjeta", size=24, weight=ft.FontWeight.BOLD),
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                
                # Campo de entrada
                card_name,
                
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                
                # Botones
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