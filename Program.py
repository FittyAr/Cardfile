from View.Login import Login
from flet import Page
import flet

""" def login_initializing(page: Page):
    page.title = "Login"
    page.vertical_alignment = flet.MainAxisAlignment.CENTER
    login=Login(page)
    page.add(login)
    page.update()

def prueba_ema(page: Page):
    pass
 """
 
def main(page: Page):
    login = Login(page)
    page.add(login)

if __name__ == "__main__":
    flet.app(target=main)