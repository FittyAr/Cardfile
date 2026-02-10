import flet as ft
from cardfile.view.wizard.pages.base_page import WizardPage


class LanguagePage(WizardPage):
    async def build_content(self):
        self.t = self.config.translations.get("wizard", {}).get("language", {
            "title": "Idioma",
            "subtitle": "Selecciona el idioma principal de la aplicación.",
        })

        language_options = [
            ft.dropdown.Option(opt["value"], opt["text"])
            for opt in self.config.get_language_options()
        ]

        def on_lang_change(e):
            selected_lang = e.control.value
            self.config.set_language(selected_lang)
            # We don't save yet, just set in memory for the rest of the wizard
            self.manager.temp_data["app.language.default"] = selected_lang
            # Trigger a re-build of the current page to update texts (optional, but good)
            # Actually, manager will handle the next page with the new language
            self.update()

        lang_dropdown = ft.Dropdown(
            label=self.t.get("label", "Idioma"),
            options=language_options,
            value=self.config.current_language,
            on_select=on_lang_change,
            width=300
        )

        return [
            ft.Text(self.t["title"], size=24, weight=ft.FontWeight.W_600),
            ft.Text(self.t["subtitle"]),
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            lang_dropdown
        ]
