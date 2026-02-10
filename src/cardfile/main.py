import flet as ft
from flet import Page
import os

from cardfile.config.Route import views_handler
from cardfile.config.auth_flow import resolve_route, normalize_route
from cardfile.config.config import Config
from cardfile.config.runtime import is_web_runtime
from cardfile.config.security import is_ip_allowed
from cardfile.data.database.setup import init_db
from cardfile.data.database.connection import get_session
from cardfile.data.models.usuario import Usuario
from cardfile.theme.manager import ThemeManager

theme_manager = ThemeManager()


def check_first_run():
    """Retorna True si la base de datos no está configurada o el archivo SQLite no existe."""
    config = Config()
    uri = config.get_database_uri()
    if uri.startswith("sqlite:///"):
        db_path = uri[len("sqlite:///"):]
        if not os.path.exists(db_path):
            return True
    return False
            
def needs_account_creation():
    """Retorna True si no existen usuarios en la base de datos."""
    try:
        session = get_session()
        user_exists = session.query(Usuario).first() is not None
        return not user_exists
    except Exception:
        return True
    finally:
        try:
            session.close()
        except:
            pass

async def main(page: Page):
    page.title = "CardFile"
    page.padding = 0
    page.spacing = 0
    config = Config()
    if is_web_runtime(page):
        client_ip = None
        try:
            client_ip = getattr(page, "client_ip", None)
        except Exception:
            client_ip = None
        allowed_ips = config.get("app.web.allowed_ips", ["0.0.0.0"])
        if not is_ip_allowed(allowed_ips, client_ip):
            page.views.clear()
            page.views.append(
                ft.View(
                    route="/",
                    controls=[
                        ft.Container(
                            content=ft.Text("Acceso restringido"),
                            alignment=ft.Alignment.CENTER,
                            expand=True
                        )
                    ],
                    vertical_alignment=ft.MainAxisAlignment.CENTER,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER
                )
            )
            page.update()
            return
    try:
        if hasattr(page, "window") and page.window:
            page.window.width = 1000
            page.window.height = 800
    except Exception:
        pass

    from cardfile.view.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)

    async def route_change(e: ft.RouteChangeEvent) -> None:
        requested_route = e.route
        is_first_run = check_first_run()
        is_authenticated = await auth_manager.is_authenticated()
        normalized_route = normalize_route(requested_route)
        resolved_route = resolve_route(
            normalized_route,
            is_authenticated,
            auth_manager.require_login,
            is_first_run,
            needs_account_creation()
        )
        if resolved_route != normalized_route:
            await page.push_route(resolved_route)
            return

        view = await views_handler(page, resolved_route)
        if view is None:
            fallback_route = "/Card" if is_authenticated else "/Login"
            await page.push_route(fallback_route)
            return

        page.views.clear()
        page.views.append(view)
        page.update()

    async def view_pop(e: ft.ViewPopEvent) -> None:
        page.views.pop()
        top_view: ft.View = page.views[-1]
        await page.push_route(top_view.route)

    page.on_route_change = route_change
    page.on_view_pop = view_pop

    is_first_run = check_first_run()
    if not is_first_run:
        from cardfile.data.database.setup import init_db
        init_db()

    initial_route = resolve_route(
        "/",
        await auth_manager.is_authenticated(),
        auth_manager.require_login,
        is_first_run,
        needs_account_creation()
    )
    await page.push_route(initial_route)

    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
