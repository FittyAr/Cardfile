import os
import flet as ft
from config.setup_db import initialize_db
from flet import Page
from config.Route import views_handler
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
        print(f"Route requested: {e.route}")
        
        # Verificar acceso antes de procesar la ruta
        if not await auth_manager.is_authenticated():
            # Si no está autenticado y no es una ruta pública, redirigir a Login
            if page.route not in ["/Login", "/newUser", "/"]:
                print(f"Not authenticated, redirecting to /Login from {page.route}")
                await page.push_route("/Login")
                return

        page.views.clear()
        # await views_handler para que pueda usar shared_preferences
        views = await views_handler(page)
        
        # Mapeo de rutas (case-insensitive) a las rutas reales del diccionario
        route_mapping = {
            '/': None,  # Se maneja especialmente
            '/card': '/Card',
            '/login': '/Login',
            '/newuser': '/newUser',
            '/newcard': '/newCard',
            '/editcard': '/editCard',
            '/recycle': '/recycle',
        }
        
        route = page.route
        
        # Manejar ruta raíz y redirecciones basadas en auth
        if not route or route == '/':
            if check_first_run():
                route = '/newUser'
            elif not auth_manager.require_login:
                route = '/Card'
            else:
                route = '/Login'
            print(f"Root route, redirecting to {route}")
        elif route.lower() == '/login' and not auth_manager.require_login:
            # Si no se requiere login, no permitir entrar a /Login
            print("Login not required, redirecting to /Card")
            await page.push_route("/Card")
            return
        else:
            # Normalizar la ruta usando el mapeo
            route_lower = route.lower()
            if route_lower in route_mapping:
                route = route_mapping[route_lower]
            
        # Si la ruta no existe, redirigir según el estado de auth
        if route not in views:
            print(f"Route {route} not found")
            if not await auth_manager.is_authenticated():
                route = '/Login'
            else:
                route = '/Card'
        
        # Limpiar vistas existentes completamente
        while len(page.views) > 0:
            page.views.pop()
        
        # Agregar la nueva vista
        page.views.append(views[route])
        page.update()

    async def view_pop(e: ft.ViewPopEvent) -> None:
        page.views.pop()
        top_view: ft.View = page.views[-1]
        await page.push_route(top_view.route)

    page.on_route_change = route_change
    page.on_view_pop = view_pop
    
    # Decidir la ruta inicial basada en si es primera ejecución
    initial_route = '/newUser' if check_first_run() else '/Login'
    await page.push_route(initial_route)

    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT

if __name__ == "__main__":
    ft.run(main)
