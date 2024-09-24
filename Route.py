import flet
from flet import Page

from View import Login, Card
from GitHub_examples.ModernNavBar import ModernNavBar

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
                ModernNavBar(page)
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