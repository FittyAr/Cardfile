import flet
from flet import Page

from Route import views_handler

def main(page: Page) -> None:
    page.title = "Fichero in Flet and Python"
    
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

    #page.theme_mode = flet.ThemeMode.LIGHT
    #UserControl_login(page)

# Se detecta si este es el archivo que se 
# esta pasando como argumento a python
if __name__ == "__main__":  
    flet.app(target=main)
    #flet.app(target=main, view=flet.WEB_BROWSER)
    #flet.app(target=main, view=flet.FLET_APP_WEB)
    #flet.app(target=main, view=flet.FLET_APP_MOBILE)