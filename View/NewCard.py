import flet as ft

def new_card_view(page: ft.Page):
    def save_clicked(e):
        if not card_name.value:
            snack = ft.SnackBar(content=ft.Text("Por favor ingrese un nombre para la tarjeta"))
            page.overlay.append(snack)
            snack.open = True
            page.update()
            return
        
        # Aquí irá la lógica para guardar en la base de datos
        print(f"Guardando tarjeta: {card_name.value}")
        page.go("/Card")  # Volver a la vista principal

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
        bgcolor=ft.colors.WHITE,
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