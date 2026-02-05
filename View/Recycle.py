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
                                color=ft.Colors.WHITE if is_selected else ft.Colors.ON_SURFACE,
                            ),
                            ft.Text(
                                f"Eliminado: {ficha.updated_at.strftime('%d/%m/%Y')}" if ficha.updated_at else "Sin fecha",
                                size=11,
                                color=ft.Colors.with_opacity(0.8, ft.Colors.WHITE) if is_selected else ft.Colors.with_opacity(0.5, ft.Colors.ON_SURFACE),
                            ),
                        ],
                        spacing=4,
                    ),
                    padding=ft.Padding.all(16),
                    border_radius=10,
                    bgcolor=ft.Colors.BLUE_400 if is_selected else ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE),
                    ink=True,
                    on_click=lambda e, f=ficha: asyncio.create_task(select_and_reload(f)),
                )
                controls.append(card_item)
            
            fichas_list.controls = controls
            if not fichas:
                fichas_list.controls = [
                    ft.Container(
                        content=ft.Text("La papelera está vacía", size=14, color=ft.Colors.with_opacity(0.5, ft.Colors.ON_SURFACE)),
                        padding=20,
                        alignment=ft.alignment.center
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
        expand=True,
        spacing=10,
        padding=0,
    )

    # Botones modernos
    btn_cancel = ft.TextButton(
        content=ft.Text(config.get_text("recycle.buttons.cancel"), color=ft.Colors.ON_SURFACE),
        on_click=lambda e: asyncio.create_task(page.push_route("/Card"))
    )

    btn_restore = ft.ElevatedButton(
        content=ft.Text(config.get_text("recycle.buttons.restore"), weight=ft.FontWeight.BOLD),
        width=130,
        height=40,
        color=ft.Colors.WHITE,
        bgcolor=ft.Colors.GREEN_400,
        style=ft.ButtonStyle(shape=ft.RoundedRectangleBorder(radius=8)),
        on_click=restore_clicked,
        disabled=True
    )

    btn_delete = ft.IconButton(
        icon=ft.Icons.DELETE_FOREVER_OUTLINED,
        icon_color=ft.Colors.RED_400,
        tooltip=config.get_text("recycle.buttons.delete"),
        on_click=delete_clicked,
        disabled=True
    )

    btn_empty_trash = ft.TextButton(
        content=ft.Row([ft.Icon(ft.Icons.DELETE_SWEEP_OUTLINED, size=18), ft.Text("Vaciar papelera")]),
        style=ft.ButtonStyle(color=ft.Colors.RED_400),
        on_click=empty_trash_clicked,
    )

    # Contenedor principal
    main_view = ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.Row([
                            ft.Icon(ft.Icons.RECYCLING_ROUNDED, color=ft.Colors.BLUE_400, size=28),
                            ft.Text(config.get_text("recycle.title"), size=24, weight=ft.FontWeight.BOLD),
                        ], spacing=10),
                        btn_empty_trash,
                    ],
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                # Lista de fichas inactivas que expande
                ft.Container(
                    content=fichas_list,
                    expand=True,
                    padding=ft.Padding.symmetric(vertical=10),
                ),
                
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                
                # Footer con acciones
                ft.Container(
                    content=ft.Row(
                        [
                            btn_cancel,
                            ft.Row([btn_delete, btn_restore], spacing=10),
                        ],
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    ),
                    padding=ft.Padding.only(top=10),
                ),
            ],
            spacing=10,
        ),
        width=600, # Más ancho para mejor visualización
        height=500,
        bgcolor=ft.Colors.SURFACE,
        border_radius=20,
        border=ft.border.all(1, ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
        padding=30,
        alignment=ft.Alignment.CENTER,
    )

    # Carga inicial
    await load_inactive_fichas()
    
    return main_view

# Exportamos la función
__all__ = ['recycle_view']
