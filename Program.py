import flet
from flet import Page

from View.Login import Login

def UserControl_login(page: Page) -> None:
    page.add(Login(page))

def main(page: Page) -> None:
    page.vertical_alignment = flet.MainAxisAlignment.CENTER
    page.horizontal_alignment = flet.MainAxisAlignment.CENTER
    page.theme_mode = flet.ThemeMode.LIGHT
    UserControl_login(page)

# Se detecta si este es el archivo que se 
# esta pasando como argumento a python
if __name__ == "__main__":  
    flet.app(target=main)
    #flet.app(target=main, view=flet.WEB_BROWSER)