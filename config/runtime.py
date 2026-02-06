import flet as ft


def is_web_runtime(page: ft.Page) -> bool:
    platform = getattr(page, "platform", None)
    if platform is not None:
        platform_name = str(platform).lower()
        if "web" in platform_name:
            return True
    return bool(getattr(page, "web", False))
