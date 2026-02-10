import flet as ft
from flet import Page

from cardfile.view.Navigation import create_navigation_bar
from cardfile.config.config import Config
from cardfile.config.runtime import is_web_runtime

async def views_handler(page: Page, route: str = None):
    """
    Crea y retorna la vista solicitada de forma lazy.
    Si route es None, retorna un diccionario con todas las vistas (para compatibilidad).
    Si route es especificado, solo crea y retorna esa vista específica.
    """
    # Importar las vistas aquí para evitar problemas de importación circular
    from cardfile.view.Card import card_view
    from cardfile.view.Login import login_view
    from cardfile.view.newUser import newUser_view
    
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['navigation']

    async def handle_navigation_change(e):
        """Maneja los cambios en la barra de navegación"""
        index = e.control.selected_index
        is_web = is_web_runtime(page)
        exit_index = 5 if not is_web else None

        if index == 0:  # Nueva tarjeta
            await page.push_route("/newCard")
        elif index == 1:  # Cambiar nombre
            prefs = ft.SharedPreferences()
            selected_ficha = await prefs.get("selected_ficha")
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
        elif index == 4:  # Configuración
            if hasattr(page, "open_settings"):
                await page.open_settings()
        elif exit_index is not None and index == exit_index:  # Salir
            from cardfile.view.components.auth_manager import AuthManager
            auth = AuthManager(page)
            await auth.logout()

    # Si se especifica una ruta, crear solo esa vista
    if route:
        if route == '/Card':
            return ft.View(
                route='/Card',
                controls=[await card_view(page)],
                vertical_alignment=ft.MainAxisAlignment.START,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                padding=ft.Padding.all(20),
            )
        elif route == '/Login':
            return ft.View(
                route='/Login',
                controls=[await login_view(page)],
                vertical_alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                spacing=26,
                padding=ft.Padding.all(20),
            )
        elif route == '/newUser':
            return ft.View(
                route='/newUser',
                controls=[await newUser_view(page)],
                vertical_alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                spacing=26,
                padding=ft.Padding.all(20),
            )
        elif route == '/Setup':
            from cardfile.view.wizard.wizard_manager import WizardManager
            async def on_wizard_complete():
                from cardfile.data.database.setup import init_db
                init_db()
                await page.push_route("/newUser")
            
            wizard = WizardManager(page, on_wizard_complete)
            await wizard.start()
            return wizard.get_view()
        else:
            return None
    
    # Para compatibilidad, si no se especifica ruta, retornar diccionario vacío
    # (el código en main.py ahora usará el parámetro route)
    return {}
