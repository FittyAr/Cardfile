from View.Login import Login
from flet import Page
import flet

def main(page: Page):
    login = Login(page)
    page.vertical_alignment = flet.MainAxisAlignment.CENTER
    page.add(login)

if __name__ == "__main__":  # Se detecta si este es el archivo que se esta pasando como argumento a python
    flet.app(target=main)