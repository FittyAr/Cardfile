import os, flet, subprocess
from config.setup_db import initialize_db
from flet import Page
from config.Route import views_handler
from data.database.setup import init_db
from data.database.connection import get_session
from data.repositories.usuario_repository import UsuarioRepository
from data.models.usuario import Usuario

def check_first_run():
    """Verifica si es la primera ejecución comprobando si hay usuarios en la BD"""
    session = get_session()
    try:
        # Intenta obtener el primer usuario
        user_exists = session.query(Usuario).first() is not None
        return not user_exists  # True si no hay usuarios (primera ejecución)
    except Exception as e:
        print(f"Error verificando usuarios: {str(e)}")
        return False
    finally:
        session.close()

def main(page: Page):
    # Actualizar la configuración de la ventana
    page.title = "CardFile"
    page.padding = 0
    page.spacing = 0
    page.window.width = 1000  # Nueva sintaxis
    page.window.height = 800  # Nueva sintaxis

    # Inicializar la base de datos si no existe
    if not os.path.exists("database.db"):
        try:
            initialize_db()
        except Exception as e:
            print(f"Error inicializando la base de datos: {str(e)}")
            return

    def route_change(e: flet.RouteChangeEvent) -> None:
        print(e.route)
        page.views.clear()
        page.views.append(
            views_handler(page)[page.route]
        )
        page.update()

    def view_pop(e: flet.ViewPopEvent) -> None:
        page.views.pop()
        top_view: flet.View = page.views[-1]
        page.go(top_view.route)

    page.on_route_change = route_change
    page.on_view_pop = view_pop
    
    # Decidir la ruta inicial basada en si es primera ejecución
    initial_route = '/newUser' if check_first_run() else '/Login'
    page.go(initial_route)

    page.theme_mode = flet.ThemeMode.SYSTEM

if __name__ == "__main__":  
    flet.app(target=main)