import flet as ft
from typing import Callable
from cardfile.config.config import Config
from cardfile.config.locking import verify_lock_password
from cardfile.theme.manager import ThemeManager

theme_manager = ThemeManager()


async def unlock_card_modal(page: ft.Page, password_hash: str, on_close: Callable, on_success: Callable):
    config = Config()
    t = config.translations["unlock_card"]

    password_field = ft.TextField(
        label=t["password_label"],
        password=True,
        can_reveal_password=True,
        width=theme_manager.input_width,
    )

    async def unlock_clicked(e):
        if not password_hash:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(t["errors"]["no_password"]),
                bgcolor=ft.Colors.RED_400,
                action=config.get_text("common.buttons.ok"),
                duration=2000
            ))
            page.update()
            return
        if not verify_lock_password(password_field.value or "", password_hash):
            page.show_dialog(ft.SnackBar(
                content=ft.Text(t["errors"]["invalid_password"]),
                bgcolor=ft.Colors.RED_400,
                action=config.get_text("common.buttons.ok"),
                duration=2000
            ))
            page.update()
            return
        await on_success()

    async def cancel_clicked(e):
        password_field.value = ""
        page.update()
        await on_close()

    return ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Icon(ft.Icons.LOCK_OUTLINED, color=theme_manager.primary, size=theme_manager.icon_size_lg),
                        ft.Text(t["title"], size=theme_manager.text_size_xxl, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                    ],
                    alignment=ft.MainAxisAlignment.CENTER,
                    spacing=theme_manager.space_12,
                ),
                ft.Divider(height=1, color=theme_manager.divider_color),
                ft.Container(height=theme_manager.space_8),
                password_field,
                ft.Container(height=theme_manager.space_8),
                ft.Container(
                    content=ft.Row(
                        [
                            ft.TextButton(
                                content=ft.Text(t["buttons"]["cancel"], color=ft.Colors.RED_400),
                                on_click=cancel_clicked,
                            ),
                            ft.Button(
                                content=ft.Text(t["buttons"]["unlock"], weight=ft.FontWeight.BOLD),
                                width=theme_manager.button_width,
                                height=theme_manager.button_height,
                                color=ft.Colors.WHITE,
                                bgcolor=theme_manager.primary,
                                style=theme_manager.primary_button_style,
                                on_click=unlock_clicked,
                            ),
                        ],
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    ),
                    padding=ft.Padding.only(top=theme_manager.space_12),
                ),
            ],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=theme_manager.space_12,
        ),
        width=theme_manager.modal_width,
        height=theme_manager.modal_height,
        padding=theme_manager.modal_padding,
        bgcolor=theme_manager.card_bg,
        border_radius=theme_manager.radius_lg,
        border=theme_manager.card_border,
        shadow=theme_manager.card_shadow,
        alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )


__all__ = ["unlock_card_modal"]
