import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from config.config import Config
import asyncio
from typing import Callable
from theme.manager import ThemeManager

theme_manager = ThemeManager()

async def recycle_modal(page: ft.Page, on_close: Callable, on_success: Callable):
    config = Config()
    selected_ficha = None
    from View.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)

    async def load_inactive_fichas():
        """Carga las fichas inactivas del usuario"""
        session = get_session()
        try:
            user_id = await auth_manager.get_authenticated_user_id()
            fichas = session.query(Ficha).filter(
                Ficha.usuario_id == user_id,
                Ficha.is_active == False
            ).all()
            
            # Crear los controles con diseño moderno similar a Card.py
            controls = []
            for ficha in fichas:
                is_selected = selected_ficha and selected_ficha.id == ficha.id
                
                card_item = ft.Container(
                    content=ft.Column(
                        [
                            ft.Text(
                                ficha.title,
                                size=14,
                                weight=ft.FontWeight.W_600,
                                color=ft.Colors.WHITE if is_selected else theme_manager.text,
                            ),
                            ft.Text(
                                f"Eliminado: {ficha.updated_at.strftime('%d/%m/%Y')}" if ficha.updated_at else "Sin fecha",
                                size=11,
                                color=ft.Colors.with_opacity(0.8, ft.Colors.WHITE) if is_selected else theme_manager.subtext,
                            ),
                        ],
                        spacing=4,
                    ),
                    padding=ft.Padding.all(16),
                    border_radius=10,
                    bgcolor=theme_manager.primary if is_selected else ft.Colors.with_opacity(0.1, theme_manager.text),
                    ink=True,
                    on_click=lambda e, f=ficha: asyncio.create_task(select_and_reload(f)),
                )
                controls.append(card_item)
            
            fichas_list.controls = controls
            if not fichas:
                fichas_list.controls = [
                    ft.Container(
                        content=ft.Text("La papelera está vacía", size=14, color=theme_manager.subtext),
                        padding=20,
                        alignment=ft.Alignment.CENTER
                    )
                ]
            
            page.update()
        except Exception as e:
            print(f"Error cargando fichas inactivas: {str(e)}")
        finally:
            session.close()

    async def select_and_reload(ficha):
        """Selecciona una ficha y recarga la lista para mostrar el resalte"""
        await select_ficha(ficha)
        await load_inactive_fichas()

    async def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        nonlocal selected_ficha
        selected_ficha = ficha
        # Habilitar botones
        btn_restore.disabled = False
        btn_delete.disabled = False
        page.update()

    async def restore_clicked(e):
        """Restaura la ficha seleccionada"""
        if not selected_ficha:
            return
        
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
            if ficha:
                ficha.is_active = True
                session.commit()
                await load_inactive_fichas()
                
                # Verificar si quedan fichas inactivas
                user_id = await auth_manager.get_authenticated_user_id()
                remaining_inactive = session.query(Ficha).filter(
                    Ficha.usuario_id == user_id,
                    Ficha.is_active == False
                ).count()
                
                page.show_dialog(ft.SnackBar(
                    content=ft.Text(config.get_text("recycle.messages.restore_success")),
                    bgcolor=ft.Colors.GREEN_400,
                    action="Ok"
                ))
                page.update()
                
                # Si no quedan fichas inactivas, volver (vía on_close/success)
                if remaining_inactive == 0:
                    await asyncio.sleep(0.5)
                    if on_success: await on_success()
                    else: await on_close()
        except Exception as e:
            session.rollback()
            print(f"Error restaurando ficha: {str(e)}")
            page.show_dialog(ft.SnackBar(
                content=ft.Text(config.get_text("recycle.messages.restore_error")),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
        finally:
            session.close()

    async def delete_clicked(e):
        """Elimina permanentemente la ficha"""
        if not selected_ficha:
            return

        async def confirm_delete(e):
            nonlocal selected_ficha
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == config.get_text("card.buttons.yes"):
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        session.delete(ficha)
                        session.commit()
                        selected_ficha = None
                        await load_inactive_fichas()
                        
                        # Verificar si quedan fichas inactivas
                        user_id = await auth_manager.get_authenticated_user_id()
                        remaining_inactive = session.query(Ficha).filter(
                            Ficha.usuario_id == user_id,
                            Ficha.is_active == False
                        ).count()
                        
                        page.show_dialog(ft.SnackBar(
                            content=ft.Text(config.get_text("recycle.messages.delete_success")),
                            bgcolor=ft.Colors.GREEN_400,
                            action="Ok"
                        ))
                        page.update()
                        
                        # Si no quedan fichas inactivas, volver
                        if remaining_inactive == 0:
                            await asyncio.sleep(0.5)
                            if on_success: await on_success()
                            else: await on_close()
                except Exception as e:
                    session.rollback()
                    print(f"Error eliminando ficha: {str(e)}")
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text(config.get_text("recycle.messages.delete_error")),
                        bgcolor=ft.Colors.RED_400,
                        action="Ok"
                    ))
                    page.update()
                finally:
                    session.close()
            
            dlg_modal.open = False
            page.update()

        dlg_modal = ft.AlertDialog(
            modal=True,
            title=ft.Text(config.get_text("recycle.delete_confirmation.title")),
            content=ft.Text(config.get_text("recycle.delete_confirmation.message")),
            actions=[
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.no")), on_click=confirm_delete),
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.yes")), on_click=confirm_delete),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )

        page.overlay.append(dlg_modal)
        dlg_modal.open = True
        page.update()

    async def empty_trash_clicked(e):
        """Vacía toda la papelera"""
        async def confirm_empty_trash(e):
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == config.get_text("card.buttons.yes"):
                session = get_session()
                try:
                    user_id = await auth_manager.get_authenticated_user_id()
                    session.query(Ficha).filter(
                        Ficha.usuario_id == user_id,
                        Ficha.is_active == False
                    ).delete()
                    session.commit()
                    await load_inactive_fichas()
                    page.show_dialog(ft.SnackBar(content=ft.Text("Papelera vaciada"), bgcolor=ft.Colors.GREEN_400))
                    page.update()
                    await asyncio.sleep(0.5)
                    if on_success: await on_success()
                    else: await on_close()
                except Exception as e:
                    session.rollback()
                    page.show_dialog(ft.SnackBar(content=ft.Text("Error al vaciar la papelera"), bgcolor=ft.Colors.RED_400))
                finally:
                    session.close()
            empty_dialog.open = False
            page.update()
        
        empty_dialog = ft.AlertDialog(
            modal=True,
            title=ft.Text("Vaciar papelera"),
            content=ft.Text("¿Borrar todo permanentemente?"),
            actions=[
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.no")), on_click=confirm_empty_trash),
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.yes")), on_click=confirm_empty_trash),
            ],
        )
        page.overlay.append(empty_dialog)
        empty_dialog.open = True
        page.update()

    async def cancel_clicked_modal(e):
        await on_close()

    fichas_list = ft.ListView(expand=True, spacing=10, padding=0)

    btn_cancel = ft.TextButton(
        content=ft.Text(config.get_text("recycle.buttons.cancel"), color=theme_manager.text),
        on_click=cancel_clicked_modal
    )

    btn_restore = ft.ElevatedButton(
        content=ft.Text(config.get_text("recycle.buttons.restore"), weight=ft.FontWeight.BOLD),
        width=130, height=40, color=ft.Colors.WHITE, bgcolor=ft.Colors.GREEN_400,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=8)),
        on_click=restore_clicked, disabled=True
    )

    btn_delete = ft.IconButton(
        icon=ft.Icons.DELETE_FOREVER_OUTLINED, icon_color=ft.Colors.RED_400,
        tooltip=config.get_text("recycle.buttons.delete"), on_click=delete_clicked, disabled=True
    )

    btn_empty_trash = ft.TextButton(
        content=ft.Row([ft.Icon(ft.Icons.DELETE_SWEEP_OUTLINED, size=18), ft.Text("Vaciar papelera")]),
        style=ft.ButtonStyle(color=ft.Colors.RED_400), on_click=empty_trash_clicked,
    )

    main_view = ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Row([
                            ft.Icon(ft.Icons.RECYCLING_ROUNDED, color=theme_manager.primary, size=28),
                            ft.Text(config.get_text("recycle.title"), size=24, weight=ft.FontWeight.BOLD, color=theme_manager.text),
                        ], spacing=10),
                        btn_empty_trash,
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                ft.Container(content=fichas_list, expand=True, padding=ft.Padding.symmetric(vertical=10)),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                ft.Container(
                    content=ft.Row([btn_cancel, ft.Row([btn_delete, btn_restore], spacing=10)], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
                    padding=ft.Padding.only(top=10),
                ),
            ],
            spacing=10,
        ),
        width=600, height=500, bgcolor=theme_manager.card_bg, border_radius=20,
        border=ft.border.all(1, ft.Colors.with_opacity(0.1, theme_manager.text)), padding=30, alignment=ft.Alignment.CENTER,
        on_click=lambda _: None,
    )

    await load_inactive_fichas()
    return main_view

# Exportamos la función
__all__ = ['recycle_modal']
