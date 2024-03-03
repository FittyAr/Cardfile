from flet import Page
import sys, flet

class Login(flet.UserControl):    # hereda de la clase user control
    def __init__(self, page: Page):
        super().__init__()        # Con esto inicializamos el constructor de la clase UserControl
        self.page = page

    def login_click(self, sender):
        print("Login Clicked")
        #self.page.alert("Login Clicked")

        """ if sys.platform == "win32": """

    def build(self):   # UserControl exige la existencia de este metodo
        login_button = flet.TextButton(text="Login", on_click=self.login_click)
        close_button = flet.TextButton(text="Close")

        container_login_button = flet.Container(login_button)        
        container_close_button = flet.Container(close_button)       

        #container_login_button.bgcolor = flet.colors.BLUE_GREY
        container_login_button.width = 150
        container_login_button.alignment = flet.alignment.center_left
        #container_close_button.bgcolor = flet.colors.BLUE_ACCENT
        container_close_button.width = 140
        container_close_button.alignment = flet.alignment.center_right

        row_buttons = flet.Row(
            [
                container_login_button, 
                container_close_button
            ]
        )

        self.login_item = [
             flet.TextField(label="Username"),
             flet.TextField(label="Password", password=True),
             row_buttons
        ]

        column  = flet.Column(self.login_item)
        column.width = 300
        column.height = 200

        container = flet.Container(column)
        container.alignment = flet.alignment.center

        return container

def main(page: Page):
    page.title = "Login"
    page.add(Login(page))

#flet.app(target=main)
#flet.app(target=main, view=flet.WEB_BROWSER)