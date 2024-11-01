import flet
from flet import Page

from View import Login, Card, Login2
#from GitHub_examples.ModernNavBar import ModernNavBar

from View.Login2 import login2_view

def views_handler(page: Page):
    return {
        '/':flet.View(
            route='/',
            controls=[
                flet.AppBar(title=flet.Text('Home'), bgcolor=flet.colors.RED),
            ],
            vertical_alignment = flet.MainAxisAlignment.CENTER,
            horizontal_alignment = flet.MainAxisAlignment.CENTER,
            spacing=26
        ),
        '/Main':flet.View(
            route='/Main',
            controls=[
                flet.AppBar(title=flet.Text('Home'), bgcolor=flet.colors.RED),
                #ModernNavBar(page)
            ],
            vertical_alignment = flet.MainAxisAlignment.CENTER,
            horizontal_alignment = flet.MainAxisAlignment.CENTER,
            spacing=26
        ),
        '/Card':flet.View(
            route='/Card',
            controls=[
                flet.AppBar(title=flet.Text('Card'), bgcolor=flet.colors.ORANGE)
            ],
            vertical_alignment = flet.MainAxisAlignment.CENTER,
            horizontal_alignment = flet.MainAxisAlignment.CENTER,
            spacing=26
        ),
        '/Login':flet.View(
            route='/Login',
            controls=[
                flet.AppBar(title=flet.Text('Login'), bgcolor=flet.colors.GREEN),
                Login.Login(page),
            ],
            vertical_alignment = flet.MainAxisAlignment.CENTER,
            horizontal_alignment = flet.MainAxisAlignment.CENTER,
            spacing=26
        ),
        '/Login2': flet.View(
            route='/Login2',
            controls=[
                flet.AppBar(title=flet.Text('Login2'), bgcolor=flet.colors.BLUE),
                login2_view(page),
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.MainAxisAlignment.CENTER,
            spacing=26
        ),
        '/Exit':flet.View(
            route='/Exit',
            controls=[
                flet.AppBar(title=flet.Text('Exit'), bgcolor=flet.colors.BLUE),
                Login.Login(page),
            ],
            vertical_alignment = flet.MainAxisAlignment.CENTER,
            horizontal_alignment = flet.MainAxisAlignment.CENTER,
            spacing=26
        )
    }