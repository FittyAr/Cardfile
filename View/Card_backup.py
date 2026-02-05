import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha
from typing import Optional
from config.config import Config
from View.MarkdownEditor import create_markdown_preview, create_markdown_toolbar
import asyncio

async def card_view(page: Page):
    # Atributos
    selected_ficha = None  # Variable para mantener la ficha seleccionada
    autosave_task: Optional[asyncio.Task] = None  # tarea de autosave periódico
    # Parámetros de guardado
    DEBOUNCE_SECONDS = 1.0
    PERIODIC_AUTOSAVE_SECONDS = 15
    # Estado de guardado
    last_saved_value: str = ""
    has_unsaved_changes = False
    debounce_task: Optional[asyncio.Task] = None
    
    fichas_list = ft.ListView(
        expand=1,
        spacing=10,
        padding=20,
        height=500
    )
    # Debounce para búsqueda
    search_debounce_task: Optional[asyncio.Task] = None
    
    # Obtener configuración y traducciones
    config = Config()
    t = config.translations['card']
    
    # Actualizar los textos estáticos
    title_label = ft.Text(
        value=t['title'],
        size=24,
        weight=ft.FontWeight.BOLD,
        color=ft.Colors.BLUE
    )

    async def description_changed():
        nonlocal has_unsaved_changes, debounce_task
        has_unsaved_changes = True
        set_status("dirty")
        if debounce_task:
            debounce_task.cancel()
        
        async def do_debounce():
            await asyncio.sleep(DEBOUNCE_SECONDS)
            await save_if_needed()
            
        debounce_task = asyncio.create_task(do_debounce())
        # Live preview cuando se edita en modo preview (switch OFF)
        if not description_text.read_only and not show_code_switch.value:
            markdown_preview.value = sanitize_for_preview(description_text.value or "")
            markdown_preview.update()

    description_text = ft.TextField(
        multiline=True,
        min_lines=16,
        max_lines=16,
        read_only=True,
        border_color=ft.Colors.BLUE_200,
        bgcolor=ft.Colors.WHITE10,
        expand=True,
        on_change=lambda e: asyncio.create_task(description_changed())
    )

    # Editor Markdown: barra + preview
    markdown_preview = create_markdown_preview()
    # Toggle para mostrar código en modo edición (se integrará en toolbar)
    def on_show_code_toggle(e):
        update_editor_visibility()
    show_code_switch = ft.Switch(
        value=False,
        on_change=on_show_code_toggle,
        active_color=ft.Colors.BLUE,
        tooltip="Mostrar/ocultar código",
        visible=False,
    )
    markdown_toolbar = create_markdown_toolbar(description_text, on_modified=description_changed, show_code_switch=show_code_switch)

    # Utilidad: saneo ligero de Markdown para preview (no modifica el valor guardado)
    import re
    def sanitize_for_preview(md: str) -> str:
        if not md:
            return ""
        # Negrita con espacios: ** texto ** -> **texto**
        md = re.sub(r"\*\*\s+([^*][^*]*?)\s+\*\*", r"**\1**", md)
        # Tachado con espacios: ~~ texto ~~ -> ~~texto~~
        md = re.sub(r"~~\s+([^~][^~]*?)\s+~~", r"~~\1~~", md)
        # Código inline con espacios: ` texto ` -> `texto`
        md = re.sub(r"`\s+([^`][^`]*?)\s+`", r"`\1`", md)
        return md

    # Toggle para mostrar código en modo edición
    def update_editor_visibility():
        # En modo lectura mostrar solo Markdown; en edición, alternar con el switch
        if description_text.read_only:
            markdown_toolbar.visible = False
            show_code_switch.visible = False
            markdown_preview.value = sanitize_for_preview(description_text.value or "")
            markdown_preview.visible = True
            description_text.visible = False
        else:
            markdown_toolbar.visible = True
            show_code_switch.visible = True
            if show_code_switch.value:
                description_text.visible = True
                markdown_preview.visible = False
            else:
                markdown_preview.value = sanitize_for_preview(description_text.value or "")
                markdown_preview.visible = True
                description_text.visible = False
        markdown_toolbar.update()
        show_code_switch.update()
        markdown_preview.update()
        description_text.update()
        page.update()

    # Estados iniciales seguros (vista en modo lectura, sin código)
    description_text.visible = False
    markdown_toolbar.visible = False
    show_code_switch.visible = False
    markdown_preview.visible = True

    # Indicador de estado de guardado
    save_status_icon = ft.Icon(ft.Icons.CHECK_CIRCLE, size=14, color=ft.Colors.GREEN_400)
    save_status_text = ft.Text("Guardado", size=12)
    save_status = ft.Row([save_status_icon, save_status_text], spacing=6, alignment=ft.MainAxisAlignment.END)

    def set_status(state: str):
        # estados: idle, dirty, saving, saved, error
        if state == "dirty":
            save_status_icon.icon = ft.Icons.EDIT
            save_status_icon.color = ft.Colors.AMBER_400
            save_status_text.value = "Cambios sin guardar"
        elif state == "saving":
            save_status_icon.icon = ft.Icons.SYNC
            save_status_icon.color = ft.Colors.BLUE_400
            save_status_text.value = "Guardando..."
        elif state == "saved":
            save_status_icon.icon = ft.Icons.CHECK_CIRCLE
            save_status_icon.color = ft.Colors.GREEN_400
            save_status_text.value = "Guardado"
        elif state == "error":
            save_status_icon.icon = ft.Icons.ERROR
            save_status_icon.color = ft.Colors.RED_400
            save_status_text.value = "Error al guardar"
        else:  # idle
            save_status_icon.icon = ft.Icons.INFO
            save_status_icon.color = ft.Colors.SURFACE_TINT
            save_status_text.value = "Listo"
        save_status.update()

    async def save_if_needed():
        nonlocal last_saved_value, has_unsaved_changes
        if not selected_ficha:
            return
        value = description_text.value or ""
        if value == last_saved_value and not has_unsaved_changes:
            return
        set_status("saving")
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
            if ficha:
                ficha.descripcion = value
                session.commit()
                last_saved_value = value
                has_unsaved_changes = False
                set_status("saved")
        except Exception as ex:
            try:
                session.rollback()
            except Exception:
                pass
            print(f"Error en guardado (debounce/autosave): {str(ex)}")
            set_status("error")
        finally:
            session.close()
            page.update()

    # Agregar el control deslizante y su función manejadora
    async def toggle_readonly(e):
        """Maneja el cambio del switch de modo lectura"""
        nonlocal autosave_task, debounce_task
        description_text.read_only = not description_text.read_only
        if not description_text.read_only:  # entra en modo edición
            set_status("idle")
            if not autosave_task:
                autosave_task = asyncio.create_task(periodic_autosave())
            edit_mode_label.value = "Modo edición"
            edit_mode_icon.icon = ft.Icons.LOCK_OPEN
            description_text.border_color = ft.Colors.AMBER_400
        else:  # vuelve a modo lectura
            if autosave_task:
                try:
                    await save_if_needed()
                finally:
                    autosave_task.cancel()
                    autosave_task = None
            edit_mode_label.value = "Modo lectura"
            edit_mode_icon.icon = ft.Icons.LOCK
            description_text.border_color = ft.Colors.BLUE_200
            if debounce_task:
                debounce_task.cancel()
                debounce_task = None
        edit_mode_label.update()
        # Forzar visibilidad consistente al cambiar modo
        if description_text.read_only:
            # Modo lectura
            show_code_switch.value = False
        update_editor_visibility()
        
    edit_mode_label = ft.Text("Modo lectura")
    edit_mode_icon = ft.Icon(ft.Icons.LOCK, color=ft.Colors.BLUE_400)
    edit_switch = ft.Row(
        controls=[
            edit_mode_icon,
            edit_mode_label,
            ft.Switch(
                value=False,
                on_change=toggle_readonly,
                active_color=ft.Colors.BLUE,
                disabled=True,
                tooltip="Activar/Desactivar edición"
            )
        ],
        alignment=ft.MainAxisAlignment.END
    )

    # Render helper
    def render_fichas_list(fichas: list[Ficha]):
        fichas_list.controls = [
            ft.ListTile(
                leading=ft.Icon(ft.Icons.DESCRIPTION),
                title=ft.Text(f.title),
                on_click=lambda e, ficha=f: asyncio.create_task(select_ficha(ficha))
            ) for f in fichas
        ]
        fichas_list.update()
        page.update()

    # Búsqueda con debounce (consulta directa a BD)
    async def fetch_and_render(search_text: str):
        search_text = (search_text or "").strip()
        session = get_session()
        try:
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            q = session.query(Ficha).filter(Ficha.usuario_id == user_id)
            if search_text:
                q = q.filter(Ficha.title.ilike(f"%{search_text}%"))
            fichas = q.all()
            render_fichas_list(fichas)
        except Exception as e:
            print(f"Error filtrando fichas: {str(e)}")
        finally:
            session.close()

    async def search_changed(e):
        nonlocal search_debounce_task
        if search_debounce_task:
            search_debounce_task.cancel()
        
        async def do_search():
            await asyncio.sleep(0.3)
            await fetch_and_render(e.control.value)
            
        search_query = e.control.value
        search_debounce_task = asyncio.create_task(do_search())

    # Actualizar el campo de búsqueda para usar la función de filtrado
    search_field = ft.TextField(
        label=t['search']['label'],
        on_change=search_changed,  # Conectar la función de filtrado con debounce
        expand=True,
        border_color=ft.Colors.BLUE_200,
        hint_text=t['search']['hint'],  # Texto de ayuda
    )

    async def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        nonlocal selected_ficha, debounce_task, last_saved_value, has_unsaved_changes
        
        # Guardar la ficha seleccionada en el almacenamiento del cliente
        # Convertir a JSON string porque shared_preferences no acepta diccionarios
        import json
        ficha_data = json.dumps({
            "id": ficha.id,
            "title": ficha.title,
            "descripcion": ficha.descripcion
        })
        await page.shared_preferences.set("selected_ficha", ficha_data)
        
        # Habilitar el switch cuando se selecciona una ficha
        edit_switch.controls[2].disabled = False
        # Inicializar estado de guardado
        last_saved_value = ficha.descripcion or ""
        has_unsaved_changes = False
        set_status("idle")
        # Estado de visibilidad inicial en modo lectura (no mostrar código)
        show_code_switch.value = False
        # Cancelar debounce pendiente
        if debounce_task:
            debounce_task.cancel()
            debounce_task = None
        
        # Si hay una ficha seleccionada anteriormente y hay cambios sin guardar
        if selected_ficha and description_text.value != selected_ficha.descripcion:
            if not description_text.read_only:  # Si estaba en modo edición
                session = get_session()
                try:
                    print("Guardando cambios pendientes...")
                    ficha_anterior = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha_anterior:
                        ficha_anterior.descripcion = description_text.value
                        session.commit()
                        print("✅ Cambios guardados antes de cambiar de ficha")
                except Exception as e:
                    session.rollback()
                    print(f"❌ Error guardando cambios pendientes: {str(e)}")
                finally:
                    session.close()

        # Obtener datos actualizados de la nueva ficha seleccionada
        session = get_session()
        try:
            ficha_actualizada = session.query(Ficha).filter(Ficha.id == ficha.id).first()
            if ficha_actualizada:
                selected_ficha = ficha_actualizada
                title_label.value = ficha_actualizada.title
                description_text.value = ficha_actualizada.descripcion
                description_text.read_only = True
                edit_switch.controls[2].value = False
                # Reset estado al cargar
                last_saved_value = ficha_actualizada.descripcion or ""
                has_unsaved_changes = False
                set_status("idle")
                # Sincronizar UI con el nuevo contenido seleccionado
                update_editor_visibility()
                print(f"✅ Ficha {ficha_actualizada.id} cargada con éxito")
        except Exception as e:
            print(f"❌ Error cargando ficha: {str(e)}")
        finally:
            session.close()
            
        page.update()

    async def delete_ficha_handler(e=None):
        nonlocal selected_ficha
        
        if not selected_ficha:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(t['delete']['no_selection']),
                bgcolor=ft.Colors.RED_400,
                action="Ok"
            ))
            page.update()
            return

        async def confirm_delete(e):
            nonlocal selected_ficha
            # En Flet 1.0, los botones usan content en lugar de text
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == t['buttons']['yes']:
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        # En lugar de eliminar, desactivamos la ficha
                        ficha.is_active = False
                        session.commit()
                        
                        # Limpiar la selección
                        selected_ficha = None
                        await page.shared_preferences.remove("selected_ficha")
                        
                        # Recargar la lista de fichas
                        await load_fichas()
                        
                        page.show_dialog(ft.SnackBar(
                            content=ft.Text(t['delete']['success']),
                            bgcolor=ft.Colors.GREEN_400,
                            action="Ok"
                        ))
                        page.update()
                    
                except Exception as e:
                    session.rollback()
                    print(f"Error desactivando ficha: {str(e)}")
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text(t['delete']['error']),
                        bgcolor=ft.Colors.RED_400,
                        action="Ok"
                    ))
                    page.update()
                finally:
                    session.close()
            
            # Cerrar el diálogo
            page.pop_dialog()
            page.update()

        # Crear el diálogo de confirmación
        dlg_modal = ft.AlertDialog(
            modal=True,
            title=ft.Text(t['delete']['confirm_title']),
            content=ft.Text(t['delete']['confirm_message']),
            actions=[
                ft.TextButton(content=ft.Text(t['buttons']['no']), on_click=confirm_delete),
                ft.TextButton(content=ft.Text(t['buttons']['yes']), on_click=confirm_delete),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )

        # Mostrar el diálogo - Flet 1.0 usa show_dialog
        page.show_dialog(dlg_modal)

    # Hacer la función accesible a nivel de página
    page.delete_ficha = delete_ficha_handler

    async def load_fichas():
        """Carga las fichas activas del usuario actual desde la base de datos"""
        try:
            session = get_session()
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            fichas = session.query(Ficha).filter(Ficha.usuario_id == user_id).all()
            render_fichas_list(fichas)
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    async def periodic_autosave():
        """Tarea de autosave periódico durante la edición"""
        while True:
            await asyncio.sleep(PERIODIC_AUTOSAVE_SECONDS)
            try:
                await save_if_needed()
            except Exception as ex:
                print(f"Error en autosave periódico: {str(ex)}")

    # Guardado forzado (atajo de teclado Ctrl+S)
    async def force_save_now(e=None):
        await save_if_needed()
        page.show_dialog(ft.SnackBar(content=ft.Text("Cambios guardados"), bgcolor=ft.Colors.GREEN_400))
        page.update()

    async def on_keyboard(e: ft.KeyboardEvent):
        try:
            key = (e.key or "").lower()
            if getattr(e, "ctrl", False) and key == "s":
                await force_save_now(e)
        except Exception:
            pass
    page.on_keyboard_event = on_keyboard

    async def clear_selection():
        """Limpia la selección actual"""
        nonlocal selected_ficha, debounce_task, last_saved_value, has_unsaved_changes
        
        await page.shared_preferences.remove("selected_ficha")
        selected_ficha = None
        title_label.value = ""
        description_text.value = ""
        description_text.read_only = True
        edit_switch.controls[2].disabled = True
        edit_switch.controls[2].value = False
        should_save = False
        page.update()

    # Panel derecho completo
    right_panel = ft.Container(
        content=ft.Column(
            controls=[
                title_label,
                edit_switch,  # Agregar el switch aquí
                save_status,
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                markdown_toolbar,
                description_text,
                markdown_preview
            ],
            spacing=10,
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
            expand=True
        ),
        padding=20,
        expand=True
    )

    # Contenedor principal
    main_view = ft.Container(
        content=ft.Row(
            controls=[
                ft.Container(
                    content=ft.Column(
                        controls=[
                            search_field,
                            ft.Divider(height=10, color=ft.Colors.TRANSPARENT),
                            fichas_list
                        ]
                    ),
                    width=300,
                    border=ft.border.all(2, ft.Colors.BLUE_200),
                    border_radius=10,
                    padding=10
                ),
                ft.VerticalDivider(width=20, color=ft.Colors.TRANSPARENT),
                ft.Container(
                    content=right_panel,
                    border=ft.border.all(2, ft.Colors.BLUE_200),
                    border_radius=10,
                    padding=10,
                    expand=True
                )
            ],
            alignment=ft.MainAxisAlignment.CENTER,
            vertical_alignment=ft.CrossAxisAlignment.START,
            expand=True
        ),
        padding=20
    )


    # Cargar fichas después de que la vista esté montada
    def on_view_mount():
        asyncio.create_task(load_fichas())

    async def on_view_unmount():
        # Guardar lo pendiente y limpiar tasks
        nonlocal autosave_task, debounce_task, search_debounce_task
        try:
            if debounce_task:
                debounce_task.cancel()
                debounce_task = None
            if search_debounce_task:
                search_debounce_task.cancel()
                search_debounce_task = None
            await save_if_needed()
        finally:
            if autosave_task:
                autosave_task.cancel()
                autosave_task = None
            # Liberar handler de teclado si corresponde a esta vista
            page.on_keyboard_event = None

    main_view.did_mount = on_view_mount
    main_view.will_unmount = on_view_unmount
    
    # Asignar handler de eliminación para que la barra de navegación pueda accederlo
    page.delete_ficha = delete_ficha_handler
    
    return main_view
