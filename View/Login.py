import sys, flet
from flet import Page
from flet_core.control_event import ControlEvent

class Login(flet.UserControl):    # hereda de la clase user control
    def __init__(self, page: Page):
        super().__init__()        # Con esto inicializamos el constructor de la clase UserControl
        self.page = page

    # , e: ControlEvent
    def login_click(self) -> None:
        print("Login Clicked")
        # print('Username: ', self.text_username.value)
        # print('Password: ', self.text_password.value)
        self.page.go('/')
        self.page.update()
        #self.page.alert("Login Clicked")

    def close_click(self) -> None:
        print("Close Clicked")
        #sys.exit()

        """ if sys.platform == "win32": """

    def Validate(self, e: ControlEvent) -> None:
        if all([self.text_username.value, self.text_password.value]):
            self.login_button.disabled = True
        else:
            self.login_button.disabled = False
        self.page.update()

    # UserControl exige la existencia de este metodo
    # retorna un objeto de tipo flet.Container
    def build(self) -> flet.Container:   
        container_login_button: flet.Container = flet.Container(self.login_button)
        container_close_button: flet.Container = flet.Container(self.close_button)
        container_check: flet.Container = flet.Container(self.checkbox_signup)

        #container_login_button.bgcolor = flet.colors.BLUE_GREY
        container_login_button.width = 150
        container_login_button.alignment = flet.alignment.center_left
        #container_close_button.bgcolor = flet.colors.BLUE_ACCENT
        container_close_button.width = 140
        container_close_button.alignment = flet.alignment.center_right
        container_check.width = 150
        container_check.alignment = flet.alignment.center

        row_buttons = flet.Row(
            [
                container_login_button, 
                container_close_button                
            ]
        )

        self.login_item = [
             self.text_username,
             self.text_password,
             container_check,
             row_buttons
        ]

        column  = flet.Column(self.login_item)
        column.width = 300
        column.height = 200

        container = flet.Container(column)
        container.alignment = flet.alignment.center

        return container

    login_button: flet.ElevatedButton = flet.ElevatedButton(text="Sign up", on_click=login_click, disabled=False)
    close_button: flet.ElevatedButton = flet.ElevatedButton(text="Close", on_click=close_click)

    text_username: flet.TextField = flet.TextField(label="Username", on_change=Validate, text_align=flet.TextAlign.LEFT)
    text_password: flet.TextField = flet.TextField(label="Password", on_change=Validate, password=True)
    checkbox_signup: flet.Checkbox = flet.Checkbox(label="Remember me", value=False)

def main(page: Page) -> None:
    page.title = "Login"
    page.add(Login(page))