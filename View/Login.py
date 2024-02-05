from flet import Page, FletApp
import sys, flet

class Login(flet.UserControl):    
    def build(self):
        app = FletApp("Login")
        
        username_box = FletTextBox("Username")
        password_box = FletTextBox("Password", password=True)
        login_button = FletButton("Login")
        
        app.add(username_box)
        app.add(password_box)
        app.add(login_button)
        
        if sys.platform == "win32":
            close_button = FletButton("Close")
            app.add(close_button)
        
        return app

def main(page: Page):
    page.title = "Login"
    page.add(Login())    
    
flet.app(tarjet=main, view=flet.WEB_BROWSER)