import sys, flet
from flet import Page
from flet_core.control_event import ControlEvent
from flet_core import Control
from data.database.setup import init_db
from data.repositories.usuario_repository import UsuarioRepository

class Login(flet.Control):    # hereda de la clase user control
    def _get_control_name(self):
        return "login"  # Nombre único para identificar este control

    def __init__(self, page: Page):
        super().__init__()
        self.page = page

        # Mover la definición de los controles al constructor
        self.login_button = flet.ElevatedButton(text="Sign up", on_click=self.login_click, disabled=False)
        self.close_button = flet.ElevatedButton(text="Close", on_click=self.close_click)
        self.text_username = flet.TextField(label="Username", on_change=self.Validate, text_align=flet.TextAlign.LEFT)
        self.text_password = flet.TextField(label="Password", on_change=self.Validate, password=True)
        self.checkbox_signup = flet.Checkbox(label="Remember me", value=False)


    # , e: ControlEvent
    def login_click(self) -> None:

        init_db()
        # Crear un repositorio de usuarios
        usuario_repo = UsuarioRepository()
        

        self.page.go('/Main')
        self.page.update()
        #self.page.alert("Login Clicked")

    def close_click(self) -> None:
        print("Close Clicked")
        self.page.go('/Card')
        self.page.update()

        """ if sys.platform == "win32": """

    def Validate(self, e: ControlEvent) -> None:
        if all([self.text_username.value, self.text_password.value]):
            self.login_button.disabled = True
        else:
            self.login_button.disabled = False
        self.page.update()

    def _build(self) -> flet.Control: 
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
        
        """
        Card = flet.Card(container)
        Card.width = 300
        Card.height = 200
        Card.elevation = 15
        """
        return container

def main(page: Page) -> None:
    page.title = "Login"
    page.add(Login(page))