import flet as ft
from cardfile.config.config import Config
from cardfile.theme.manager import ThemeManager
from cardfile.view.wizard.pages.welcome_page import WelcomePage
from cardfile.view.wizard.pages.language_page import LanguagePage
from cardfile.view.wizard.pages.storage_page import StoragePage
from cardfile.view.wizard.pages.database_page import DatabasePage
from cardfile.view.wizard.pages.finish_page import FinishPage


class WizardManager:
    def __init__(self, page: ft.Page, on_complete):
        self.page = page
        self.on_complete = on_complete
        self.config = Config()
        self.theme = ThemeManager()
        self.is_advanced = False
        self.temp_data = {
            "is_portable": False  # Quick mode defaults to Standard storage
        }
        
        # Define the pages flow
        self.all_pages = [
            WelcomePage,    # Intro + Mode selection
            LanguagePage,   # i18n
            StoragePage,    # Portable vs Standard
            DatabasePage,   # DB connection / init
            FinishPage      # Summary
        ]
        
        self.current_page_index = 0
        self.current_page_instance = None
        self.container = ft.Container(expand=True)

    async def start(self):
        await self.show_page(0)

    async def show_page(self, index):
        if 0 <= index < len(self.all_pages):
            self.current_page_index = index
            page_class = self.all_pages[index]
            
            # Filter pages based on mode (Quick mode skips some advanced config)
            if not self.is_advanced:
                # Example: skip Storage and Database if quick mode (use defaults)
                if page_class in [StoragePage, DatabasePage]:
                    await self.show_page(index + 1)
                    return

            self.current_page_instance = page_class(self)
            await self.current_page_instance.on_enter()
            
            self.container.content = self.current_page_instance
            await self.update_navigation()
            self.page.update()

    async def next_page(self, e=None):
        if await self.current_page_instance.validate():
            await self.current_page_instance.on_leave()
            if self.current_page_index < len(self.all_pages) - 1:
                await self.show_page(self.current_page_index + 1)
            else:
                await self.finish()

    async def prev_page(self, e=None):
        if self.current_page_index > 0:
            await self.show_page(self.current_page_index - 1)

    async def update_navigation(self):
        # This will be handled by the layout wrapper
        pass

    async def finish(self):
        # Persist all data from temp_data to Config
        for key, value in self.temp_data.items():
            self.config.set(key, value)
        self.config.save_config()
        await self.on_complete()

    def get_view(self):
        return ft.View(
            route="/Setup",
            controls=[
                ft.Container(
                    content=ft.Column([
                        ft.Text(self.config.get_text("wizard.title"), 
                               size=32, weight=ft.FontWeight.BOLD),
                        self.container,
                        ft.Row([
                            ft.Button(
                                self.config.get_text("common.buttons.back"),
                                on_click=self.prev_page,
                                visible=self.current_page_index > 0
                            ),
                            ft.Button(
                                self.config.get_text("common.buttons.next"),
                                on_click=self.next_page,
                                color=ft.Colors.WHITE,
                                bgcolor=self.theme.primary
                            ),
                        ], alignment=ft.MainAxisAlignment.END)
                    ], expand=True),
                    padding=40,
                    expand=True
                )
            ]
        )
