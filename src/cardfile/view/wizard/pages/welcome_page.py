import flet as ft
from cardfile.view.wizard.pages.base_page import WizardPage
from cardfile.config.runtime import get_os_platform


class WelcomePage(WizardPage):
    async def build_content(self):
        self.t = self.config.translations.get("wizard", {}).get("welcome", {})

        is_docker = get_os_platform() == 'docker'
        
        # If docker, force advanced and default to it
        if is_docker:
            self.manager.is_advanced = True
            initial_value = "advanced"
        else:
            initial_value = "quick"

        def on_mode_change(e):
            self.manager.is_advanced = (e.control.value == "advanced")
            self.update()

        mode_selection = ft.RadioGroup(
            content=ft.Column([
                ft.Container(
                    content=ft.Column([
                        ft.Radio(
                            value="quick", 
                            label=self.t["quick_title"], 
                            fill_color=self.theme.primary,
                            disabled=is_docker
                        ),
                        ft.Text(self.t["quick_desc"], size=12, color=self.theme.subtext),
                        ft.Text(self.t["docker_notice"], size=11, color=ft.Colors.ORANGE_400, visible=is_docker),
                    ]),
                    padding=10,
                    border=ft.Border.all(1, self.theme.divider_color),
                    border_radius=10,
                    opacity=0.5 if is_docker else 1.0
                ),
                ft.Container(
                    content=ft.Column([
                        ft.Radio(value="advanced", label=self.t["adv_title"], fill_color=self.theme.primary),
                        ft.Text(self.t["adv_desc"], size=12, color=self.theme.subtext),
                    ]),
                    padding=10,
                    border=ft.Border.all(1, self.theme.divider_color),
                    border_radius=10,
                ),
            ], spacing=10),
            value=initial_value,
            on_change=on_mode_change
        )

        return [
            ft.Text(self.t["title"], size=24, weight=ft.FontWeight.W_600),
            ft.Text(self.t["subtitle"]),
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            ft.Text(self.t["mode_question"], weight=ft.FontWeight.BOLD),
            mode_selection
        ]
