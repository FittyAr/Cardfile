import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha
import threading
import time
from typing import Optional
from config.config import Config
from View.MarkdownEditor import create_markdown_preview, create_markdown_toolbar

def card_view(page: Page):
    # Atributos
    selected_ficha = None  # Variable para mantener la ficha seleccionada
    save_thread: Optional[threading.Thread] = None  # hilo de autosave periódico
    should_save = False  # indicador de autosave en ejecución
    # Parámetros de guardado
    DEBOUNCE_SECONDS = 1.0
    PERIODIC_AUTOSAVE_SECONDS = 15
    # Estado de guardado
    last_saved_value: str = ""
    has_unsaved_changes = False
    debounce_timer: Optional[threading.Timer] = None
    
    fichas_list = ft.ListView(
        expand=1,
        spacing=10,
        padding=20,
        height=500
    )
    # Caché y debounce para búsqueda
    fichas_cache: list[Ficha] = []
    search_debounce_timer: Optional[threading.Timer] = None
    
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

    def description_changed():
        nonlocal has_unsaved_changes, debounce_timer
        has_unsaved_changes = True
        set_status("dirty")
        if debounce_timer:
            try:
                debounce_timer.cancel()
            except Exception:
                pass
        debounce_timer = threading.Timer(DEBOUNCE_SECONDS, save_if_needed)
        debounce_timer.daemon = True
        debounce_timer.start()

    description_text = ft.TextField(
        multiline=True,
        min_lines=16,
        max_lines=16,
        read_only=True,
        border_color=ft.Colors.BLUE_200,
        bgcolor=ft.Colors.WHITE10,
        expand=True,
        on_change=lambda e: description_changed()
    )

    # Editor Markdown: barra + preview
    markdown_preview = create_markdown_preview()
    markdown_toolbar = create_markdown_toolbar(description_text, on_modified=description_changed)

    # Indicador de estado de guardado
    save_status_icon = ft.Icon(ft.Icons.CHECK_CIRCLE, size=14, color=ft.Colors.GREEN_400)
    save_status_text = ft.Text("Guardado", size=12)
    save_status = ft.Row([save_status_icon, save_status_text], spacing=6, alignment=ft.MainAxisAlignment.END)

    def set_status(state: str):
        # estados: idle, dirty, saving, saved, error
        if state == "dirty":
            save_status_icon.name = ft.Icons.EDIT
            save_status_icon.color = ft.Colors.AMBER_400
            save_status_text.value = "Cambios sin guardar"
        elif state == "saving":
            save_status_icon.name = ft.Icons.SYNC
            save_status_icon.color = ft.Colors.BLUE_400
            save_status_text.value = "Guardando..."
        elif state == "saved":
            save_status_icon.name = ft.Icons.CHECK_CIRCLE
            save_status_icon.color = ft.Colors.GREEN_400
            save_status_text.value = "Guardado"
        elif state == "error":
            save_status_icon.name = ft.Icons.ERROR
            save_status_icon.color = ft.Colors.RED_400
            save_status_text.value = "Error al guardar"
        else:  # idle
            save_status_icon.name = ft.Icons.INFO
            save_status_icon.color = ft.Colors.SURFACE_TINT
            save_status_text.value = "Listo"
        save_status.update()

    def save_if_needed():
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
    def toggle_readonly(e):
        """Maneja el cambio del switch de modo lectura"""
        nonlocal save_thread, should_save, debounce_timer
        description_text.read_only = not description_text.read_only
        if not description_text.read_only:  # entra en modo edición
            set_status("idle")
            should_save = True
            if not save_thread:
                save_thread = threading.Thread(target=periodic_autosave)
                save_thread.daemon = True
                save_thread.start()
            edit_mode_label.value = "Modo edición"
            edit_mode_icon.name = ft.Icons.LOCK_OPEN
            description_text.border_color = ft.Colors.AMBER_400
        else:  # vuelve a modo lectura
            should_save = False
            if save_thread:
                try:
                    save_if_needed()
                finally:
                    save_thread.join(timeout=1.1)
                    save_thread = None
            edit_mode_label.value = "Modo lectura"
            edit_mode_icon.name = ft.Icons.LOCK
            description_text.border_color = ft.Colors.BLUE_200
            if debounce_timer:
                try:
                    debounce_timer.cancel()
                except Exception:
                    pass
                debounce_timer = None
        edit_mode_label.update()
        # Alternar visibilidad de controles según modo (ver/edición)
        if description_text.read_only:
            markdown_toolbar.visible = False
            markdown_preview.value = description_text.value or ""
            markdown_preview.visible = True
        else:
            markdown_toolbar.visible = True
            markdown_preview.visible = False
        markdown_toolbar.update()
        markdown_preview.update()
        description_text.update()
        
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

    # Agregar el campo de búsqueda con debounce y caché
    def apply_filter_and_render(search_text: str):
        search_text = (search_text or "").lower().strip()
        if not search_text:
            filtered = fichas_cache
        else:
            filtered = [f for f in fichas_cache if (f.title or "").lower().find(search_text) >= 0]
        fichas_list.controls = [
            ft.ListTile(
                leading=ft.Icon(ft.Icons.DESCRIPTION),
                title=ft.Text(f.title),
                on_click=lambda e, ficha=f: select_ficha(ficha)
            ) for f in filtered
        ]
        fichas_list.update()
        page.update()

    def search_changed(e):
        nonlocal search_debounce_timer
        if search_debounce_timer:
            try:
                search_debounce_timer.cancel()
            except Exception:
                pass
        query = e.control.value
        search_debounce_timer = threading.Timer(0.3, lambda: apply_filter_and_render(query))
        search_debounce_timer.daemon = True
        search_debounce_timer.start()

    # Actualizar el campo de búsqueda para usar la función de filtrado
    search_field = ft.TextField(
        label=t['search']['label'],
        on_change=search_changed,  # Conectar la función de filtrado con debounce
        expand=True,
        border_color=ft.Colors.BLUE_200,
        hint_text=t['search']['hint'],  # Texto de ayuda
    )

    def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        nonlocal selected_ficha, should_save, debounce_timer, last_saved_value, has_unsaved_changes
        
        # Guardar la ficha seleccionada en el almacenamiento del cliente
        page.client_storage.set("selected_ficha", {
            "id": ficha.id,
            "title": ficha.title,
            "descripcion": ficha.descripcion
        })
        
        # Habilitar el switch cuando se selecciona una ficha
        edit_switch.controls[2].disabled = False
        # Inicializar estado de guardado
        last_saved_value = ficha.descripcion or ""
        has_unsaved_changes = False
        set_status("idle")
        # Cancelar debounce pendiente
        if debounce_timer:
            try:
                debounce_timer.cancel()
            except Exception:
                pass
            debounce_timer = None
        
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
                should_save = False
                # Reset estado al cargar
                last_saved_value = ficha_actualizada.descripcion or ""
                has_unsaved_changes = False
                set_status("idle")
                print(f"✅ Ficha {ficha_actualizada.id} cargada con éxito")
        except Exception as e:
            print(f"❌ Error cargando ficha: {str(e)}")
        finally:
            session.close()
            
        page.update()

    def delete_ficha_handler(e=None):
        nonlocal selected_ficha
        
        if not selected_ficha:
            page.snack_bar = ft.SnackBar(
                content=ft.Text(t['delete']['no_selection']),
                bgcolor=ft.Colors.RED_400,
                action=t['buttons']['ok']
            )
            page.snack_bar.open = True
            page.update()
            return

        def confirm_delete(e):
            nonlocal selected_ficha
            if e.control.text == t['buttons']['yes']:
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        # En lugar de eliminar, desactivamos la ficha
                        ficha.is_active = False
                        session.commit()
                        
                        # Limpiar la selección y deshabilitar controles
                        clear_selection()
                        
                        # Recargar la lista de fichas
                        load_fichas()
                        
                        page.snack_bar = ft.SnackBar(
                            content=ft.Text(t['delete']['success']),
                            bgcolor=ft.Colors.GREEN_400,
                            action=t['buttons']['ok']
                        )
                        page.snack_bar.open = True
                        page.update()
                    
                except Exception as e:
                    session.rollback()
                    print(f"Error desactivando ficha: {str(e)}")
                    page.snack_bar = ft.SnackBar(
                        content=ft.Text(t['delete']['error']),
                        bgcolor=ft.Colors.RED_400,
                        action=t['buttons']['ok']
                    )
                    page.snack_bar.open = True
                    page.update()
                finally:
                    session.close()
            
            # Cerrar el diálogo
            dlg_modal.open = False
            page.update()

        # Crear el diálogo de confirmación
        dlg_modal = ft.AlertDialog(
            modal=True,
            title=ft.Text(t['delete']['confirm_title']),
            content=ft.Text(t['delete']['confirm_message']),
            actions=[
                ft.TextButton(t['buttons']['no'], on_click=confirm_delete),
                ft.TextButton(t['buttons']['yes'], on_click=confirm_delete),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )

        # Mostrar el diálogo
        page.dialog = dlg_modal
        dlg_modal.open = True
        page.update()

    # Hacer la función accesible a nivel de página
    page.delete_ficha = delete_ficha_handler

    def load_fichas():
        """Carga las fichas activas del usuario actual desde la base de datos"""
        try:
            session = get_session()
            user_id = page.client_storage.get("user_id")
            # Cachear todas las fichas del usuario (activas e inactivas)
            cached = session.query(Ficha).filter(
                Ficha.usuario_id == user_id
            ).all()
            fichas_cache.clear()
            fichas_cache.extend(cached)
            # Render según el texto actual del buscador
            apply_filter_and_render(search_field.value)
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    def periodic_autosave():
        """Hilo de autosave periódico durante la edición"""
        nonlocal should_save
        while should_save:
            time.sleep(PERIODIC_AUTOSAVE_SECONDS)
            try:
                save_if_needed()
            except Exception as ex:
                print(f"Error en autosave periódico: {str(ex)}")

    # Guardado forzado (atajo de teclado Ctrl+S)
    def force_save_now(e=None):
        save_if_needed()
        page.snack_bar = ft.SnackBar(content=ft.Text("Cambios guardados"), bgcolor=ft.Colors.GREEN_400)
        page.snack_bar.open = True
        page.update()

    def on_keyboard(e: ft.KeyboardEvent):
        try:
            key = (e.key or "").lower()
            if getattr(e, "ctrl", False) and key == "s":
                force_save_now(e)
        except Exception:
            pass
    page.on_keyboard_event = on_keyboard

    def clear_selection():
        """Limpia la selección actual"""
        nonlocal selected_ficha, should_save
        
        page.client_storage.remove("selected_ficha")
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
        load_fichas()

    def on_view_unmount():
        # Guardar lo pendiente y limpiar timers/hilos y handlers
        nonlocal save_thread, should_save, debounce_timer, search_debounce_timer
        try:
            if debounce_timer:
                try:
                    debounce_timer.cancel()
                except Exception:
                    pass
                debounce_timer = None
            if search_debounce_timer:
                try:
                    search_debounce_timer.cancel()
                except Exception:
                    pass
                search_debounce_timer = None
            save_if_needed()
        finally:
            should_save = False
            if save_thread:
                try:
                    save_thread.join(timeout=0.3)
                except Exception:
                    pass
                save_thread = None
            # Liberar handler de teclado si corresponde a esta vista
            page.on_keyboard_event = None

    main_view.did_mount = on_view_mount
    main_view.will_unmount = on_view_unmount
    return main_view