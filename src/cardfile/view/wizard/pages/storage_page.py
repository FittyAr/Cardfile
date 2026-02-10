import flet as ft
from cardfile.view.wizard.pages.base_page import WizardPage
from cardfile.config.runtime import get_os_platform, get_data_dir


class StoragePage(WizardPage):
    async def build_content(self):
        os_platform = get_os_platform()
        self.t = self.config.translations.get("wizard", {}).get("storage", {})
        
        # Subtitle usually contains the detected platform
        subtitle = self.t.get("subtitle", "Plataforma detectada: {platform}").format(platform=os_platform.capitalize())

        is_docker = os_platform == 'docker'
        
        if is_docker:
            # En docker siempre preferimos el modo estándar que apunta a /app
            self.manager.temp_data["is_portable"] = False
            path = get_data_dir("Cardfile", portable=False)
        else:
            path = get_data_dir("Cardfile", portable=self.manager.config.is_portable)

        def on_storage_change(e):
            is_portable = (e.control.value == "portable")
            self.manager.temp_data["is_portable"] = is_portable
            path = get_data_dir("Cardfile", portable=is_portable)
            path_preview.value = self.t["path_preview"].format(path=path)
            self.update()
        
        path_preview = ft.Text(self.t["path_preview"].format(path=path), size=12, italic=True)

        storage_selection = ft.RadioGroup(
            content=ft.Column([
                ft.Container(
                    content=ft.Column([
                        ft.Radio(
                            value="portable", 
                            label=self.t["portable_title"], 
                            fill_color=self.theme.primary,
                            disabled=is_docker
                        ),
                        ft.Text(self.t["portable_desc"], size=12, color=self.theme.subtext),
                        ft.Text(self.t["docker_recommend"], size=11, color=ft.Colors.ORANGE_400, visible=is_docker),
                    ]),
                    padding=10,
                    border=ft.Border.all(1, self.theme.divider_color),
                    border_radius=10,
                    opacity=0.5 if is_docker else 1.0
                ),
                ft.Container(
                    content=ft.Column([
                        ft.Radio(value="standard", label=self.t["standard_title"], fill_color=self.theme.primary),
                        ft.Text(self.t["standard_desc"], size=12, color=self.theme.subtext),
                    ]),
                    padding=10,
                    border=ft.Border.all(1, self.theme.divider_color),
                    border_radius=10,
                ),
            ], spacing=10),
            value="standard" if is_docker else ("portable" if self.config.is_portable else "standard"),
            on_change=on_storage_change
        )

        return [
            ft.Text(self.t["title"], size=24, weight=ft.FontWeight.W_600),
            ft.Text(subtitle),
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            ft.Text(self.t["docker_recommend"], size=11, color=ft.Colors.ORANGE_400, visible=is_docker),
            ft.Text(self.t["question"], weight=ft.FontWeight.BOLD),
            storage_selection,
            path_preview
        ]
