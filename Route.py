import flet
from flet import Page

from View import Login, Card
from View.Card import card_view
#from GitHub_examples.ModernNavBar import ModernNavBar

from View import Login
from View.Login import login_view
from View.newUser import newUser_view

def views_handler(page: Page):
    return {
        '/': flet.View(
            route='/',
            controls=[
                flet.AppBar(
                    title=flet.Text('Inicio', size=20, weight=flet.FontWeight.BOLD),
                    bgcolor=flet.colors.BLUE,
                    center_title=True,
                    actions=[
                        flet.IconButton(flet.icons.BRIGHTNESS_4, on_click=lambda _: page.theme_mode == "dark"),
                    ]
                ),
                # Contenedor centrado para el contenido
                flet.Container(
                    content=flet.Column(
                        controls=[],
                        horizontal_alignment=flet.CrossAxisAlignment.CENTER,
                    ),
                    alignment=flet.alignment.center,
                )
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            padding=20,
        ),
        '/Main': flet.View(
            route='/Main',
            controls=[
                flet.AppBar(title=flet.Text('Home'), bgcolor=flet.colors.RED),
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26
        ),
        '/Card': flet.View(
            route='/Card',
            controls=[
                flet.AppBar(
                    title=flet.Text('Mis Fichas', size=20, weight=flet.FontWeight.BOLD),
                    bgcolor=flet.colors.BLUE,
                    center_title=True
                ),
                card_view(page)
            ],
            vertical_alignment=flet.MainAxisAlignment.START,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            padding=20
        ),
        '/Login': flet.View(
            route='/Login',
            controls=[
                flet.AppBar(title=flet.Text('Login'), bgcolor=flet.colors.BLUE),
                login_view(page),
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
        '/newUser': flet.View(
            route='/newUser',
            controls=[
                flet.AppBar(title=flet.Text('Nuevo Usuario'), bgcolor=flet.colors.BLUE),
                newUser_view(page),
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
    }