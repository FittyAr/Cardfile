import flet
from flet import Page

from View import Login, Card, newUser
from View.Card import card_view
from View.Login import login_view
from View.newUser import newUser_view
from View.NewCard import new_card_view
from View.EditCard import edit_card_view
from View.Recycle import recycle_view

def views_handler(page: Page):
    def edit_ficha():
        """Navega a la vista de edición si hay una ficha seleccionada"""
        # Obtener la ficha seleccionada del almacenamiento del cliente
        selected_ficha = page.client_storage.get("selected_ficha")
        if selected_ficha:
            page.go("/editCard")
        else:
            page.show_snack_bar(
                flet.SnackBar(
                    content=flet.Text("Por favor seleccione una ficha para editar"),
                    bgcolor=flet.colors.RED_400,
                    action="Ok"
                )
            )

    return {
        '/Card': flet.View(
            route='/Card',
            controls=[
                card_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.START,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            padding=20,
            floating_action_button=flet.Row(
                controls=[
                    flet.FloatingActionButton(
                        icon=flet.icons.ADD,
                        on_click=lambda _: page.go("/newCard"),
                        tooltip="Agregar nueva tarjeta",
                        bgcolor=flet.colors.BLUE,
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.EDIT,
                        on_click=lambda _: edit_ficha(),
                        tooltip="Editar",
                        bgcolor=flet.colors.GREEN,
                        disabled=True,
                        data="edit_button"
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.DELETE,
                        on_click=lambda e: page.delete_ficha(),
                        tooltip="Eliminar",
                        bgcolor=flet.colors.RED,
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.RECYCLING,    
                        on_click=lambda _: page.go("/recycle"),
                        tooltip="Papelera de reciclaje",
                        bgcolor=flet.colors.GREEN,
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.EXIT_TO_APP,
                        on_click=lambda _: page.go("/Login"),
                        tooltip="Salir",
                        bgcolor=flet.colors.RED,
                    )
                ],
                alignment=flet.MainAxisAlignment.END,
                spacing=10,
                expand=True
            )
        ),
        '/Login': flet.View(
            route='/Login',
            controls=[
                login_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
        '/newUser': flet.View(
            route='/newUser',
            controls=[
                newUser_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
        '/newCard': flet.View(  # Agregar la nueva ruta
            route='/newCard',
            controls=[
                new_card_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
        '/editCard': flet.View(
            route='/editCard',
            controls=[
                edit_card_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
        '/recycle': flet.View(
            route='/recycle',
            controls=[
                recycle_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
    }