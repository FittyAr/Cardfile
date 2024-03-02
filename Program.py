from View.Login import Login
from flet import Page
import flet

def login_initializing(page: Page):
    page.title = "Login"
    page.vertical_alignment = flet.MainAxisAlignment.CENTER
    login=Login(page)
    page.add(login)
    page.update()

def main():
    flet.app(target=login_initializing)

if __name__ == "__main__":
    main()