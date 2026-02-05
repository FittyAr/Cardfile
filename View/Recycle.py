import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from config.config import Config
import asyncio

async def recycle_view(page: ft.Page):
    # Inicializar Config
    config = Config()
    selected_ficha = None

    async def load_inactive_fichas():
        """Carga las fichas inactivas del usuario"""
        session = get_session()
        try:
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            fichas = session.query(Ficha).filter(
                Ficha.usuario_id == user_id,
                Ficha.is_active == False
            ).all()
            
            # Crear los controles antes de asignarlos
            controls = [
                ft.ListTile(
                    leading=ft.Icon(ft.Icons.DELETE_OUTLINE),
                    title=ft.Text(ficha.title),
                    on_click=lambda e, f=ficha: asyncio.create_task(select_ficha(f))
                ) for ficha in fichas
            ]
            
            # Asignar los controles al ListView
            fichas_list.controls = controls
            # Solo actualizar la página si el control ya está montado
            try:
                page.update()
            except Exception:
                pass  # El control aún no está en la página
        except Exception as e:
            print(f"Error cargando fichas inactivas: {str(e)}")
        finally:
            session.close()

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
                user_id = await page.shared_preferences.get("user_id")
                user_id = int(user_id) if user_id else None
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
                
                # Si no quedan fichas inactivas, volver a /Card
                if remaining_inactive == 0:
                    await asyncio.sleep(0.5)  # Pequeña pausa para que se vea el mensaje
                    await page.push_route("/Card")
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
            # En Flet 1.0, los botones usan content en lugar de text
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
                        user_id = await page.shared_preferences.get("user_id")
                        user_id = int(user_id) if user_id else None
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
                        
                        # Si no quedan fichas inactivas, volver a /Card
                        if remaining_inactive == 0:
                            await asyncio.sleep(0.5)
                            await page.push_route("/Card")
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
            
            # Cerrar diálogo
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

        # Abrir diálogo con API de Flet 0.80.0
        page.overlay.append(dlg_modal)
        dlg_modal.open = True
        page.update()

    async def empty_trash_clicked(e):
        """Vacía toda la papelera (elimina todas las fichas inactivas)"""
        async def confirm_empty_trash(e):
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == config.get_text("card.buttons.yes"):
                session = get_session()
                try:
                    user_id = await page.shared_preferences.get("user_id")
                    user_id = int(user_id) if user_id else None
                    
                    # Eliminar todas las fichas inactivas del usuario
                    deleted_count = session.query(Ficha).filter(
                        Ficha.usuario_id == user_id,
                        Ficha.is_active == False
                    ).delete()
                    
                    session.commit()
                    await load_inactive_fichas()
                    
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text(f"Se eliminaron {deleted_count} ficha(s) permanentemente"),
                        bgcolor=ft.Colors.GREEN_400,
                        action="Ok"
                    ))
                    page.update()
                    
                    # Volver a /Card después de vaciar la papelera
                    await asyncio.sleep(0.5)
                    await page.push_route("/Card")
                except Exception as e:
                    session.rollback()
                    print(f"Error vaciando papelera: {str(e)}")
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text("Error al vaciar la papelera"),
                        bgcolor=ft.Colors.RED_400,
                        action="Ok"
                    ))
                    page.update()
                finally:
                    session.close()
            
            # Cerrar diálogo
            empty_dialog.open = False
            page.update()
        
        empty_dialog = ft.AlertDialog(
            modal=True,
            title=ft.Text("Vaciar papelera"),
            content=ft.Text("¿Estás seguro de que deseas eliminar TODAS las fichas de la papelera permanentemente? Esta acción no se puede deshacer."),
            actions=[
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.no")), on_click=confirm_empty_trash),
                ft.TextButton(content=ft.Text(config.get_text("card.buttons.yes")), on_click=confirm_empty_trash),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )
        
        # Abrir diálogo
        page.overlay.append(empty_dialog)
        empty_dialog.open = True
        page.update()

    async def cancel_clicked(e):
        await page.push_route("/Card")

    # Lista de fichas
    fichas_list = ft.ListView(
        expand=1,
        spacing=10,
        padding=20,
        height=300
    )

    # Botones
    btn_cancel = ft.ElevatedButton(
        content=ft.Text(config.get_text("recycle.buttons.cancel")),
        width=120,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.BLUE,
        on_click=lambda e: asyncio.create_task(page.push_route("/Card"))
    )

    btn_restore = ft.ElevatedButton(
        content=ft.Text(config.get_text("recycle.buttons.restore")),
        width=120,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.GREEN,
        on_click=restore_clicked,
        disabled=True
    )

    btn_delete = ft.ElevatedButton(
        content=ft.Text(config.get_text("recycle.buttons.delete")),
        width=100,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.RED,
        on_click=delete_clicked,
        disabled=True
    )

    btn_empty_trash = ft.ElevatedButton(
        content=ft.Text("Vaciar papelera"),
        width=140,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.ORANGE_700,
        on_click=empty_trash_clicked,
    )

    # Contenedor principal
    main_view = ft.Container(
        width=400,
        height=500,
        bgcolor=ft.Colors.WHITE10,
        border=ft.border.all(2, ft.Colors.BLUE_200),
        border_radius=15,
        padding=ft.Padding.all(30),
        content=ft.Column(
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            spacing=20,
            controls=[
                ft.Icon(ft.Icons.RECYCLING, size=50, color=ft.Colors.BLUE),
                ft.Text(
                    config.get_text("recycle.title"),
                    size=28,
                    weight=ft.FontWeight.BOLD
                ),
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                
                # Lista de fichas inactivas
                ft.Container(
                    content=fichas_list,
                    border=ft.border.all(2, ft.Colors.BLUE_200),
                    border_radius=10,
                    padding=10,
                    expand=True
                ),
                
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                
                # Botón vaciar papelera
                ft.Container(
                    content=btn_empty_trash,
                ),
                
                ft.Divider(height=10, color=ft.Colors.TRANSPARENT),
                
                # Contenedor para los botones individuales
                ft.Container(
                    content=ft.Row(
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                        controls=[btn_cancel, btn_restore, btn_delete],
                    ),
                ),
            ],
        ),
        alignment=ft.Alignment.CENTER,
    )

    # Carga inicial
    await load_inactive_fichas()
    
    return main_view

# Exportamos la función
__all__ = ['recycle_view']
