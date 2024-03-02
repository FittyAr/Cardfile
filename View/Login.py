from flet import Page
import sys, flet

class Login(flet.UserControl):    
    def __init__(self, page: Page):
        super().__init__()
        self.page = page
        
    def build(self):
        self.login_item = [
             flet.TextField(label="Username"),
             flet.TextField(label="Password", password=True),
             flet.TextButton(text="Login")
        ]
        
        if sys.platform == "win32":
            close_button = flet.TextButton(text="Close")
            self.login_item.append(close_button)
        
        return flet.Column(self.login_item)

def main(page: Page):
    page.title = "Login"
    login=Login(page)
    page.add(login)
    page.update()    
    
#app(target=main)
flet.app(target=main, view=flet.WEB_BROWSER)