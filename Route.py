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
    def handle_navigation_change(e):
        """Maneja los cambios en la barra de navegación"""
        index = e.control.selected_index
        
        if index == 0:  # Nueva tarjeta
            page.go("/newCard")
        elif index == 1:  # Editar
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
        elif index == 2:  # Eliminar
            if hasattr(page, 'delete_ficha'):
                page.delete_ficha()
        elif index == 3:  # Papelera
            page.go("/recycle")
        elif index == 4:  # Salir
            page.go("/Login")

    return {
        '/Card': flet.View(
            route='/Card',
            controls=[
                card_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.START,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            padding=20,
            navigation_bar=flet.NavigationBar(
                selected_index=0,
                destinations=[
                    flet.NavigationBarDestination(
                        icon=flet.icons.ADD,
                        selected_icon=flet.icons.ADD_CIRCLE,
                        label="Nueva",
                    ),
                    flet.NavigationBarDestination(
                        icon=flet.icons.EDIT,
                        selected_icon=flet.icons.EDIT_ROUNDED,
                        label="Editar",
                    ),
                    flet.NavigationBarDestination(
                        icon=flet.icons.DELETE,
                        selected_icon=flet.icons.DELETE_FOREVER,
                        label="Eliminar",
                    ),
                    flet.NavigationBarDestination(
                        icon=flet.icons.RECYCLING,
                        selected_icon=flet.icons.RECYCLING_ROUNDED,
                        label="Papelera",
                    ),
                    flet.NavigationBarDestination(
                        icon=flet.icons.EXIT_TO_APP,
                        selected_icon=flet.icons.EXIT_TO_APP_ROUNDED,
                        label="Salir",
                    ),
                ],
                on_change=handle_navigation_change,
                bgcolor=flet.colors.SURFACE_VARIANT,
                height=65,
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