from flet import Page, FletApp
import sys, flet

class Login(flet.UserControl):    
    def __init__(self, page: Page):        
        app = FletApp("Login")
        
        username_box = flet.TextField(label="Username")
        password_box = flet.TextField(label="Password", password=True)
        login_button = flet.TextButton(text="Login")
        
        app.add(username_box)
        app.add(password_box)
        app.add(login_button)
        
        if sys.platform == "win32":
            close_button = flet.TextButton(text="Close")
            app.add(close_button)
        
        return app

def main(page: Page):
    page.title = "Login"
    login=Login()
    page.add(login)    
    
flet.app(target=main, view=flet.WEB_BROWSER)