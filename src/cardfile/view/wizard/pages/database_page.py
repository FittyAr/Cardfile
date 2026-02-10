import flet as ft
from cardfile.view.wizard.pages.base_page import WizardPage
from cardfile.config.runtime import get_os_platform


class DatabasePage(WizardPage):
    async def build_content(self):
        self.t = self.config.translations.get("wizard", {}).get("database", {})

        os_platform = get_os_platform()
        is_docker = os_platform == 'docker'

        def on_db_change(e):
            db_file = e.control.value
            if not db_file.startswith("sqlite:///"):
                uri = f"sqlite:///{db_file}"
            else:
                uri = db_file
            self.manager.temp_data["database.uri"] = uri
            uri_preview.value = self.t["uri_preview"].format(uri=uri)
            self.update()

        # Recommendation for Docker
        if is_docker:
            initial_file = "/app/database.db"
            initial_uri = f"sqlite:///{initial_file}"
            self.manager.temp_data["database.uri"] = initial_uri
        else:
            initial_uri = self.config.get("database.uri", "sqlite:///database.db")
            initial_file = initial_uri.replace("sqlite:///", "")

        db_file_input = ft.TextField(
            label=self.t["sqlite_label"],
            value=initial_file,
            on_change=on_db_change,
            width=400
        )

        uri_preview = ft.Text(self.t["uri_preview"].format(uri=initial_uri), size=12, italic=True)

        return [
            ft.Text(self.t["title"], size=24, weight=ft.FontWeight.W_600),
            ft.Text(self.t["subtitle"]),
            ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
            ft.Text(self.t["docker_recommend"], size=11, color=ft.Colors.ORANGE_400, visible=is_docker),
            db_file_input,
            uri_preview
        ]
