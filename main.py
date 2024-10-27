import os, flet, subprocess
from setup_db import initialize_db
from flet import Page

from Route import views_handler

from data.database.setup import init_db
from data.repositories.usuario_repository import UsuarioRepository
from data.models.usuario import Usuario

# Verifica si la base de datos ya está inicializada
if not os.path.exists("database.db"):
    #por ahora unico modo que encontre para que no entre en un bucle infinito. 
    #se envia una unica ves a un sub proceso para que lo ejecute por fuera de flet
    subprocess.run(["python", "setup_db.py"], check=True)
    #initialize_db()

def main(page: Page) -> None:    
    page.title = "Card archiving with Flet and Python"
    page.horizontal_alignment = flet.CrossAxisAlignment.CENTER
    page.vertical_alignment = flet.MainAxisAlignment.CENTER

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
    page.go('/Login')

    page.theme_mode = flet.ThemeMode.SYSTEM
    #UserControl_login(page)

# Se detecta si este es el archivo que se 
# esta pasando como argumento a python
if __name__ == "__main__":  
    flet.app(target=main)
    #flet.app(target=main, view=flet.AppView.WEB_BROWSER)
    #flet.app(target=main, view=flet.AppView.FLET_APP_WEB)
    #flet.app(target=main, view=flet.FLET_APP_MOBILE)