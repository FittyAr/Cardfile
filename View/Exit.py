import flet
from flet import Page
from flet_core.control_event import ControlEvent

class Exit(flet.UserControl):
    def __init__(self, page: Page):
        super().__init__()        # Con esto inicializamos el constructor de la clase UserControl
        self.page = page
        
    def build(self):
        self.page.window.prevent_close = True
        self.page.window.on_event = self.window_event
        
        self.confirm_dialog = flet.AlertDialog(
            modal=True,
            title=flet.Text("Please confirm"),
            content=flet.Text("Do you really want to exit this app?"),
            actions=[
                flet.ElevatedButton("Yes", on_click=self.yes_click),
                flet.OutlinedButton("No", on_click=self.no_click),
            ],
            actions_alignment=flet.MainAxisAlignment.END,
        )
        self.page.add(flet.Text('Try exiting this app by clicking window\'s "Close" button!'))
        return self

    def window_event(self, e: ControlEvent):
        if e.data == "close":
            self.page.dialog = self.confirm_dialog
            self.confirm_dialog.open = True
            self.page.update()
            
    def yes_click(self, e):
        self.page.window.destroy()

    def no_click(self, e):
        self.confirm_dialog.open = False
        self.page.update()
        
def main(page: Page):
    exit_control = Exit(page)
    page.add(exit_control)