import os
import flet as ft
from config.setup_db import initialize_db
from flet import Page
from config.Route import views_handler
from config.auth_flow import resolve_route, normalize_route
from data.database.setup import init_db
from data.database.connection import get_session
from data.repositories.usuario_repository import UsuarioRepository
from data.models.usuario import Usuario
from theme.manager import ThemeManager

theme_manager = ThemeManager()

# Inicializar la base de datos al importar el módulo
init_db()

def check_first_run():
    """Verifica si es la primera ejecución comprobando si hay usuarios en la BD"""
    session = get_session()
    try:
        # Intenta obtener el primer usuario
        user_exists = session.query(Usuario).first() is not None
        return not user_exists  # True si no hay usuarios (primera ejecución)
    except Exception as e:
        print(f"Error verificando usuarios: {str(e)}")
        return True  # Si hay error, asumir primera ejecución
    finally:
        session.close()

async def main(page: Page):
    # Actualizar la configuración de la ventana
    page.title = "CardFile"
    page.padding = 0
    page.spacing = 0
    # En Flet 1.0, las propiedades de ventana pueden haber cambiado
    # Si estamos en modo web con Browser, estas propiedades pueden no aplicarse
    try:
        if hasattr(page, 'window') and page.window:
            page.window.width = 1000
            page.window.height = 800
    except Exception:
        # En modo web, las propiedades de ventana pueden no estar disponibles
        pass

    from View.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)

    async def route_change(e: ft.RouteChangeEvent) -> None:
        requested_route = e.route
        is_first_run = check_first_run()
        is_authenticated = await auth_manager.is_authenticated()
        normalized_route = normalize_route(requested_route)
        resolved_route = resolve_route(
            normalized_route,
            is_authenticated,
            auth_manager.require_login,
            is_first_run
        )
        if resolved_route != normalized_route:
            await page.push_route(resolved_route)
            return

        # Crear solo la vista necesaria de forma lazy
        view = await views_handler(page, resolved_route)
        if view is None:
            # Si la vista no existe, redirigir según autenticación
            fallback_route = "/Card" if is_authenticated else "/Login"
            await page.push_route(fallback_route)
            return
        
        page.views.clear()
        page.views.append(view)
        page.update()

    async def view_pop(e: ft.ViewPopEvent) -> None:
        page.views.pop()
        top_view: ft.View = page.views[-1]
        await page.push_route(top_view.route)

    page.on_route_change = route_change
    page.on_view_pop = view_pop
    
    # Decidir la ruta inicial basada en si es primera ejecución
    initial_route = resolve_route(
        "/",
        await auth_manager.is_authenticated(),
        auth_manager.require_login,
        check_first_run()
    )
    await page.push_route(initial_route)

    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT

if __name__ == "__main__":
    ft.run(main)
