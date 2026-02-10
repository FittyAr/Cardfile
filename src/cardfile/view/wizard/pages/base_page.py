import flet as ft
from abc import ABC, abstractmethod


class WizardPage(ft.Column, ABC):
    def __init__(self, manager):
        super().__init__(spacing=20, expand=True)
        self.manager = manager
        self.config = manager.config
        self.theme = manager.theme
        self.t = self.config.translations.get("wizard", {})

    @abstractmethod
    async def build_content(self):
        """Build the UI content for the page."""
        pass

    async def validate(self) -> bool:
        """Validate input before moving to the next page."""
        return True

    async def on_enter(self):
        """Called when the page is displayed."""
        self.controls = await self.build_content()

    async def on_leave(self):
        """Called when leaving the page."""
        pass
