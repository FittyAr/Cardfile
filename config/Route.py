import flet as ft
from flet import Page

from View import Login, Card, newUser
from View.Card import card_view
from View.Login import login_view
from View.newUser import newUser_view
from View.NewCard import new_card_view
from View.EditCard import edit_card_view
from View.Recycle import recycle_view
from View.Navigation import create_navigation_bar
from config.config import Config

def views_handler(page: Page):
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['navigation']

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
                    ft.SnackBar(
                        content=ft.Text(t['errors']['select_card_edit']),
                        bgcolor=ft.colors.RED_400,
                        action=t['buttons']['ok']
                    )
                )
        elif index == 2:  # Eliminar
            if hasattr(page, 'delete_ficha'):
                page.delete_ficha()
        elif index == 3:  # Papelera
            page.go("/recycle")
        elif index == 4:  # Salir
            page.go("/Login")

    # Crear diccionario de vistas
    return {
        '/Card': ft.View(
            route='/Card',
            controls=[card_view(page)],
            vertical_alignment=ft.MainAxisAlignment.START,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            padding=20,
            navigation_bar=create_navigation_bar(page, handle_navigation_change)
        ),
        '/Login': ft.View(
            route='/Login',
            controls=[login_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.padding.all(20),
        ),
        '/newUser': ft.View(
            route='/newUser',
            controls=[newUser_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.padding.all(20),
        ),
        '/newCard': ft.View(
            route='/newCard',
            controls=[new_card_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.padding.all(20),
        ),
        '/editCard': ft.View(
            route='/editCard',
            controls=[edit_card_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.padding.all(20),
        ),
        '/recycle': ft.View(
            route='/recycle',
            controls=[recycle_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.padding.all(20),
        ),
    }