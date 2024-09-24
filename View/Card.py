import flet
from flet import Page
from flet_core.control_event import ControlEvent

class Card(flet.UserControl):
    def __init__(self, page: Page):
        super().__init__()
        self.page = page

    def build(self) -> flet.Container:
        # Panel izquierdo con ListView
        list_view = flet.ListView(
            items=[
                flet.ListTile(text="Item 1"),
                flet.ListTile(text="Item 2"),
                flet.ListTile(text="Item 3"),
                flet.ListTile(text="Item 4"),
            ],
            width=200
        )

        # Panel derecho con Label y cuadro de texto de varias líneas
        right_panel = flet.Column(
            controls=[
                flet.Text(value="Título", size=20, weight="bold"),
                flet.TextField(
                    multiline=True,
                    value="Este es un cuadro de texto de varias líneas.",
                    width=300,
                    height=200
                )
            ],
            alignment=flet.alignment.center,
            spacing=10
        )

        # Contenedor principal con el VerticalDivider
        main_container = flet.Row(
            controls=[
                list_view,
                flet.VerticalDivider(),
                right_panel
            ],
            alignment=flet.alignment.center,
            spacing=10
        )

        return main_container

def main(page: Page) -> None:
    page.title = "Main Page"
    page.add(Card(page))