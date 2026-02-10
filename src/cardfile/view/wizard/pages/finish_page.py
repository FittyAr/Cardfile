import flet as ft
from cardfile.view.wizard.pages.base_page import WizardPage



class FinishPage(WizardPage):
    async def build_content(self):
        self.t = self.config.translations.get("wizard", {}).get("finish", {
            "title": "¡Todo listo!",
            "subtitle": "Hemos recopilado toda la información necesaria.",
            "summary_title": "Resumen de configuración:",
            "lang_label": "Idioma: {val}",
            "storage_label": "Modo Almacenamiento: {val}",
            "db_label": "Base de Datos: {val}",
            "confirm_btn": "Finalizar Instalación"
        })

        lang = self.manager.temp_data.get("app.language.default", self.config.current_language)
        is_portable = self.manager.temp_data.get("is_portable", self.config.is_portable)
        db_uri = self.manager.temp_data.get("database.uri", self.config.get("database.uri", "sqlite:///database.db"))

        summary = ft.Column([
            ft.Text(self.t["summary_title"], weight=ft.FontWeight.BOLD),
            ft.Text(self.t["lang_label"].format(val=lang)),
            ft.Text(self.t["storage_label"].format(val="Portable" if is_portable else "Estándar")),
            ft.Text(self.t["db_label"].format(val=db_uri)),
        ], spacing=5)

        return [
            ft.Text(self.t["title"], size=24, weight=ft.FontWeight.W_600),
            ft.Text(self.t["subtitle"]),
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            summary,
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            ft.Text("Haz clic en el botón de la derecha para aplicar los cambios e iniciar la aplicación.")
        ]


