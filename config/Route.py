import flet as ft
from flet import Page

from View.Navigation import create_navigation_bar
from config.config import Config

async def views_handler(page: Page):
    # Importar las vistas aquí para evitar problemas de importación circular
    from View.Card import card_view
    from View.Login import login_view
    from View.newUser import newUser_view
    
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['navigation']

    async def handle_navigation_change(e):
        """Maneja los cambios en la barra de navegación"""
        index = e.control.selected_index
        
        if index == 0:  # Nueva tarjeta
            await page.push_route("/newCard")
        elif index == 1:  # Cambiar nombre
            selected_ficha = await page.shared_preferences.get("selected_ficha")
            if selected_ficha:
                await page.push_route("/editCard")
            else:
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(t['errors']['select_card_edit']),
                    bgcolor=ft.Colors.RED_400,
                    action="Ok"
                ))
                page.update()
        elif index == 2:  # Eliminar
            if hasattr(page, 'delete_ficha'):
                await page.delete_ficha()
        elif index == 3:  # Papelera
            await page.push_route("/recycle")
        elif index == 4:  # Salir
            from View.components.auth_manager import AuthManager
            auth = AuthManager(page)
            await auth.logout()

    # Crear diccionario de vistas
    return {
        '/Card': ft.View(
            route='/Card',
            controls=[await card_view(page)],
            vertical_alignment=ft.MainAxisAlignment.START,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            padding=ft.Padding.all(20),
        ),
        '/Login': ft.View(
            route='/Login',
            controls=[await login_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.Padding.all(20),
        ),
        '/newUser': ft.View(
            route='/newUser',
            controls=[await newUser_view(page)],
            vertical_alignment=ft.MainAxisAlignment.CENTER,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=ft.Padding.all(20),
        ),
    }
