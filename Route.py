import flet
from flet import Page

from View import Login, Card, newUser
from View.Card import card_view
from View.Login import login_view
from View.newUser import newUser_view
from View.NewCard import new_card_view

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
            padding=20,
            floating_action_button=flet.Row(
                controls=[
                    flet.FloatingActionButton(
                        icon=flet.icons.ADD,
                        on_click=lambda _: page.go("/newCard"),
                        tooltip="Agregar nueva tarjeta",
                        bgcolor=flet.colors.BLUE,
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.EDIT,
                        on_click=lambda _: page.go("/newCard"),
                        tooltip="Editar",
                        bgcolor=flet.colors.GREEN,
                    ),
                    flet.FloatingActionButton(
                        icon=flet.icons.EXIT_TO_APP,
                        on_click=lambda _: page.go("/Login"),
                        tooltip="Salir",
                        bgcolor=flet.colors.RED,
                    )
                ],
                alignment=flet.MainAxisAlignment.END,
                spacing=10
            )
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
        '/newCard': flet.View(  # Agregar la nueva ruta
            route='/newCard',
            controls=[
                flet.AppBar(title=flet.Text('Nueva Tarjeta'), bgcolor=flet.colors.BLUE),
                new_card_view(page),
            ],
            vertical_alignment=flet.MainAxisAlignment.CENTER,
            horizontal_alignment=flet.CrossAxisAlignment.CENTER,
            spacing=26,
            padding=flet.padding.all(20),
        ),
    }