from typing import Any, List
import flet
from flet import Page

class Layout(flet.UserControl):
    def __init__(self, page: Page):
        super().__init__()
        self.page = page

    def Title (self) -> flet.Container:
        flet.AppBar(title=flet.Text('Home'), bgcolor=flet.colors.RED)

    def NavBar (self) -> flet.Container:
        pass

    def Footer (self) -> flet.Container:
        pass

    def Body (self) -> flet.Container:
        pass

    def build(self) -> flet.Container: 
        return super().build()