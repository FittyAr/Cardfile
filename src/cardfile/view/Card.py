import flet as ft
from cardfile.data.database.connection import get_session
from cardfile.data.models.ficha import Ficha
from cardfile.data.models.usuario import Usuario
from datetime import datetime
from cardfile.config.config import Config
from cardfile.config.locking import get_user_locking_settings, mask_title
import asyncio
from cardfile.theme.manager import ThemeManager

theme_manager = ThemeManager()

# Importar componentes modularizados
from cardfile.view.components.markdown_editor import (
    create_markdown_editor,
    create_markdown_toolbar,
    create_markdown_preview
)
from cardfile.view.components.card_ui import (
    create_sidebar,
    create_card_header,
    create_custom_tabs,
    create_save_indicator,
    create_card_counter,
    create_cards_listview,
    create_search_field
)
from cardfile.view.components.card_state import CardState
from cardfile.view.components.auth_manager import AuthManager

# Importar componentes de modales
from cardfile.view.NewCard import new_card_modal
from cardfile.view.EditCard import edit_card_modal
from cardfile.view.Recycle import recycle_modal
from cardfile.view.Settings import settings_modal
from cardfile.view.UnlockCard import unlock_card_modal

async def card_view(page: ft.Page):
    """Vista moderna de tarjetas con diseño profesional tipo dashboard"""
    config = Config()
    t = config.translations['card']
    # Aplicar modo oscuro/claro según el tema
    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
    
    # Estado centralizado
    state = CardState()
    search_query = ""

    auth_manager = AuthManager(page)
    if auth_manager.require_login and not await auth_manager.is_authenticated():
        await page.push_route("/Login")
        return ft.Container()

    user_id = await auth_manager.get_authenticated_user_id()
    session = get_session()
    try:
        current_user = session.query(Usuario).filter(Usuario.id == user_id).first()
    finally:
        session.close()
    locking_settings = get_user_locking_settings(config, current_user)
    locking_enabled = locking_settings["enabled"]
    auto_lock_seconds = locking_settings["auto_lock_seconds"]
    mask_visible_chars = locking_settings["mask_visible_chars"]
    lock_password_hash = locking_settings["password_hash"]
    
    # ==================== COMPONENTES DE UI ====================
    
    # --- Modales ---
    async def hide_modal():
        modal_overlay.visible = False
        modal_overlay.content = ft.Container()
        page.update()
        await asyncio.sleep(0)
        modal_overlay.content = None
        page.update()

    async def on_modal_success():
        await hide_modal()
        await load_fichas(search_field.value)
    
    # --- Handlers ---
    async def on_editor_change(e):
        """Maneja cambios en el editor"""
        state.mark_as_modified()
        markdown_preview.value = markdown_editor.value
        if state.debounce_task: state.debounce_task.cancel()
        state.debounce_task = asyncio.create_task(debounced_save())

    async def debounced_save():
        await asyncio.sleep(2)
        await save_current_ficha()

    async def on_search_change(e):
        await load_fichas(search_field.value)

    def on_editor_tab_click(e):
        editor_container.visible = True
        preview_container.visible = False
        update_editor_state()
        editor_btn.bgcolor = theme_manager.primary
        editor_btn.content.color = ft.Colors.WHITE
        preview_btn.bgcolor = theme_manager.subtle_bg
        preview_btn.content.color = theme_manager.text
        page.update()

    def ficha_is_locked(ficha):
        if not locking_enabled or not ficha:
            return False
        return ficha.is_locked and ficha.id not in state.unlocked_fichas

    def get_display_title(ficha):
        if not ficha:
            return ""
        if locking_enabled and ficha.is_locked and ficha.id not in state.unlocked_fichas:
            return mask_title(ficha.title, mask_visible_chars)
        return ficha.title

    def on_preview_tab_click(e):
        editor_container.visible = False
        preview_container.visible = True
        markdown_toolbar.visible = False
        markdown_preview.value = markdown_editor.value
        preview_btn.bgcolor = theme_manager.primary
        preview_btn.content.color = ft.Colors.WHITE
        editor_btn.bgcolor = theme_manager.subtle_bg
        editor_btn.content.color = theme_manager.text
        page.update()

    async def edit_card_handler(e):
        """Handler para el botón de cambiar nombre de tarjeta"""
        if state.selected_ficha:
            modal_content = await edit_card_modal(page, on_close=hide_modal, on_success=on_modal_success)
            modal_overlay.content = modal_content
            modal_overlay.visible = True
            page.update()

    async def new_card_handler(e):
        """Handler para el botón de nueva tarjeta en el sidebar"""
        modal_content = await new_card_modal(page, on_close=hide_modal, on_success=on_modal_success)
        modal_overlay.content = modal_content
        modal_overlay.visible = True
        page.update()

    async def settings_handler(e):
        async def settings_success():
            await hide_modal()
            new_content = await card_view(page)
            if page.views:
                page.views[-1].controls = [new_content]
            page.update()

        modal_content = await settings_modal(page, on_close=hide_modal, on_success=settings_success)
        modal_overlay.content = modal_content
        modal_overlay.visible = True
        page.update()

    async def delete_ficha_handler(e=None):
        """Handler para el botón de eliminar del header"""
        await delete_ficha_logic()

    async def toggle_lock_handler(e=None):
        if not locking_enabled or not state.selected_ficha:
            return
        ficha = state.selected_ficha
        if ficha.is_locked:
            async def unlock_persistent():
                await set_ficha_lock_state(ficha.id, False)
            await open_unlock_modal(ficha, unlock_persistent)
        else:
            await set_ficha_lock_state(ficha.id, True)

    async def recycle_bin_handler(e):
        modal_content = await recycle_modal(page, on_close=hide_modal, on_success=on_modal_success)
        modal_overlay.content = modal_content
        modal_overlay.visible = True
        page.update()

    async def logout_handler(e):
        """Handler para el botón de cerrar sesión"""
        await auth_manager.logout()

    # --- Componentes ---
    search_field = create_search_field(on_search_change)
    card_counter = create_card_counter()
    cards_listview = create_cards_listview()
    markdown_editor = create_markdown_editor(on_change=on_editor_change)
    markdown_preview = create_markdown_preview()
    markdown_toolbar = create_markdown_toolbar(markdown_editor, on_change=on_editor_change)
    save_indicator = create_save_indicator()
    
    selected_card_title = ft.Text(
        "Selecciona una tarjeta",
        size=theme_manager.text_size_xxl, weight=ft.FontWeight.BOLD, color=theme_manager.text
    )

    tabs_row, editor_btn, preview_btn = create_custom_tabs(
        on_editor_tab_click, on_preview_tab_click
    )

    header_container, lock_header_btn, edit_header_btn, delete_header_btn = create_card_header(
        selected_card_title, 
        save_indicator,
        edit_callback=edit_card_handler,
        delete_callback=delete_ficha_handler,
        lock_callback=toggle_lock_handler
    )

    editor_container = ft.Container(content=markdown_editor, expand=True, visible=False)
    preview_container = ft.Container(
        content=ft.Column([markdown_preview], scroll=ft.ScrollMode.AUTO, expand=True),
        padding=ft.Padding.all(theme_manager.space_20), expand=True, visible=True
    )
    on_preview_tab_click(None)
    
    # --- Overlay Modal con Blur ---
    modal_overlay = ft.Container(
        content=None,
        bgcolor=theme_manager.modal_overlay_bg,
        blur=ft.Blur(theme_manager.modal_overlay_blur, theme_manager.modal_overlay_blur),
        visible=False,
        expand=True,
        alignment=ft.Alignment.CENTER,
        # La propiedad on_click aquí cerraría el modal al tocar fuera si el contenido no ocupa todo
        on_click=lambda _: asyncio.create_task(hide_modal())
    )
    
    # ==================== FUNCIONES ====================
    
    def update_editor_state():
        """Actualiza el estado de habilitación del editor basado en la selección"""
        # Usar el objeto state en lugar de variables separadas
        editor_enabled = state.editor_should_be_enabled() and not ficha_is_locked(state.selected_ficha)
        
        # Actualizar estado de componentes
        if markdown_editor:
            markdown_editor.disabled = not editor_enabled
        if markdown_toolbar:
            markdown_toolbar.visible = editor_enabled and editor_container.visible
        
        # Actualizar mensaje del título
        if not state.has_fichas():
            selected_card_title.value = "📝 No hay tarjetas"
        elif not state.is_ficha_selected():
            selected_card_title.value = "👉 Selecciona una tarjeta"
        else:
            selected_card_title.value = get_display_title(state.selected_ficha)
        
        # Deshabilitar tabs y botones de acción si no hay selección
        editor_btn.disabled = not editor_enabled
        preview_btn.disabled = not editor_enabled
        edit_header_btn.disabled = not editor_enabled
        delete_header_btn.disabled = not editor_enabled
        if locking_enabled:
            lock_header_btn.visible = True
            lock_header_btn.disabled = not state.is_ficha_selected()
            if state.selected_ficha and state.selected_ficha.is_locked:
                lock_header_btn.icon = ft.Icons.LOCK
                lock_header_btn.tooltip = "Desbloquear tarjeta"
            else:
                lock_header_btn.icon = ft.Icons.LOCK_OPEN
                lock_header_btn.tooltip = "Bloquear tarjeta"
        else:
            lock_header_btn.visible = False
        
        page.update()
    
    async def load_fichas(search_text=""):
        """Carga las fichas del usuario"""
        session = get_session()
        try:
            # Obtener el ID del usuario actual (real o Guest)
            user_id = await auth_manager.get_authenticated_user_id()
            
            # Filtro estricto por usuario para aislamiento de datos
            q = session.query(Ficha).filter(Ficha.usuario_id == user_id, Ficha.is_active == True)
            if search_text:
                q = q.filter(Ficha.title.ilike(f"%{search_text}%"))
            fichas = q.all()
            state.fichas_list = fichas  # Usar state object
            render_fichas_list(fichas)
            
            # Actualizar contador
            card_counter.value = f"{len(fichas)} tarjeta{'s' if len(fichas) != 1 else ''}"
            
            # Intentar restaurar selección previa
            prefs = ft.SharedPreferences()
            selected_ficha_data = await prefs.get("selected_ficha")
            if selected_ficha_data:
                import json
                try:
                    data = json.loads(selected_ficha_data)
                    for ficha in fichas:
                        if ficha.id == data.get("id"):
                            await select_ficha(ficha)
                            break
                except:
                    pass
            
            # Actualizar estado del editor
            update_editor_state()
            page.update()
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    def cancel_relock_task(ficha_id):
        task = state.relock_tasks.pop(ficha_id, None)
        if task and not task.done():
            task.cancel()

    async def set_ficha_lock_state(ficha_id, locked):
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == ficha_id).first()
            if ficha:
                ficha.is_locked = locked
                session.commit()
            for item in state.fichas_list:
                if item.id == ficha_id:
                    item.is_locked = locked
                    break
            if locked:
                state.unlocked_fichas.discard(ficha_id)
                cancel_relock_task(ficha_id)
            else:
                state.unlocked_fichas.discard(ficha_id)
            render_fichas_list(state.fichas_list)
            update_editor_state()
            page.update()
        except Exception as e:
            session.rollback()
            print(f"Error actualizando bloqueo: {str(e)}")
        finally:
            session.close()

    async def schedule_relock(ficha_id):
        if auto_lock_seconds <= 0:
            await set_ficha_lock_state(ficha_id, True)
            return
        cancel_relock_task(ficha_id)
        async def relock_task():
            await asyncio.sleep(auto_lock_seconds)
            await set_ficha_lock_state(ficha_id, True)
        state.relock_tasks[ficha_id] = asyncio.create_task(relock_task())

    async def open_unlock_modal(ficha, on_success):
        async def unlock_success():
            await hide_modal()
            state.unlocked_fichas.add(ficha.id)
            cancel_relock_task(ficha.id)
            await on_success()

        modal_content = await unlock_card_modal(page, lock_password_hash, on_close=hide_modal, on_success=unlock_success)
        modal_overlay.content = modal_content
        modal_overlay.visible = True
        page.update()
    
    def render_fichas_list(fichas):
        """Renderiza la lista de tarjetas en el sidebar"""
        cards_listview.controls.clear()
        
        for ficha in fichas:
            # Tarjeta individual con diseño moderno
            title_text = ft.Text(
                get_display_title(ficha),
                size=theme_manager.text_size_md,
                weight=ft.FontWeight.W_600,
                color=theme_manager.text,
                max_lines=1,
                no_wrap=True,
                overflow=ft.TextOverflow.ELLIPSIS,
            )
            date_text = ft.Text(
                f"Actualizado: {ficha.updated_at.strftime('%d/%m/%Y')}" if ficha.updated_at else "Sin fecha",
                size=theme_manager.text_size_sm,
                color=theme_manager.subtext,
                max_lines=1,
                no_wrap=True,
                overflow=ft.TextOverflow.ELLIPSIS,
            )
            lock_indicator = None
            if locking_enabled and ficha.is_locked:
                is_unlocked = ficha.id in state.unlocked_fichas
                lock_indicator = ft.Icon(
                    ft.Icons.LOCK_OPEN if is_unlocked else ft.Icons.LOCK,
                    color=theme_manager.primary if is_unlocked else theme_manager.subtext,
                    size=theme_manager.icon_size_md,
                )

            row_controls = [
                ft.Column(
                    [
                        title_text,
                        date_text,
                    ],
                    spacing=theme_manager.space_4,
                    expand=True,
                )
            ]
            if lock_indicator:
                row_controls.append(lock_indicator)

            card_item = ft.Container(
                content=ft.Row(
                    row_controls,
                    expand=True,
                    alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                ),
                padding=ft.Padding.all(theme_manager.space_16),
                border_radius=theme_manager.radius_md,
                bgcolor=theme_manager.selection_bg,
                ink=True,
                on_click=lambda e, f=ficha: asyncio.create_task(select_ficha(f)),
                expand=True,
                data=ficha.id,
            )
            cards_listview.controls.append(card_item)
        
        page.update()
    
    async def select_ficha(ficha, force_unlock=False):
        """Selecciona una tarjeta"""
        if locking_enabled and ficha.is_locked and ficha.id not in state.unlocked_fichas and not force_unlock:
            async def unlock_then_select():
                await select_ficha(ficha, True)
            await open_unlock_modal(ficha, unlock_then_select)
            return

        # Guardar cambios pendientes de la tarjeta anterior
        if state.selected_ficha and state.has_unsaved_changes:
            await save_current_ficha()

        if locking_enabled and state.selected_ficha and state.selected_ficha.is_locked and state.selected_ficha.id in state.unlocked_fichas:
            await schedule_relock(state.selected_ficha.id)
        
        state.select_ficha(ficha)  # Usar método del state
        if locking_enabled and ficha.is_locked:
            state.unlocked_fichas.add(ficha.id)
            cancel_relock_task(ficha.id)
        
        # Guardar en shared_preferences
        import json
        ficha_data = json.dumps({
            "id": ficha.id,
            "title": ficha.title,
            "descripcion": ficha.descripcion
        })
        prefs = ft.SharedPreferences()
        await prefs.set("selected_ficha", ficha_data)
        
        # Actualizar UI
        selected_card_title.value = get_display_title(ficha)
        markdown_editor.value = ficha.descripcion or ""
        markdown_preview.value = ficha.descripcion or ""
        on_preview_tab_click(None)
        
        # Resaltar tarjeta seleccionada
        for control in cards_listview.controls:
            is_selected = control.data == ficha.id
            control.bgcolor = theme_manager.primary if is_selected else theme_manager.selection_bg
            column = control.content.controls[0]
            column.controls[0].color = theme_manager.selected_text if is_selected else theme_manager.text
            column.controls[1].color = theme_manager.selected_subtext if is_selected else theme_manager.subtext
        
        # Actualizar estado del editor
        update_editor_state()
        page.update()
    
    async def save_current_ficha():
        """Guarda la tarjeta actual"""
        if not state.selected_ficha:
            return
        
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == state.selected_ficha.id).first()
            if ficha:
                ficha.descripcion = markdown_editor.value
                session.commit()
                state.mark_as_saved(markdown_editor.value)  # Usar método del state
                
                # Mostrar indicador de guardado
                save_indicator.visible = True
                page.update()
                await asyncio.sleep(2)
                save_indicator.visible = False
                page.update()
        except Exception as e:
            session.rollback()
            print(f"Error guardando ficha: {str(e)}")
        finally:
            session.close()
    

    
    async def delete_ficha_logic(e=None):
        """Lógica para eliminar la tarjeta seleccionada"""
        if not state.selected_ficha:
            page.show_dialog(ft.SnackBar(
                content=ft.Text(t['delete']['no_selection']),
                bgcolor=ft.Colors.RED_400,
                action="Ok",
                duration=2000
            ))
            page.update()
            return
        
        async def confirm_delete(e):
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == t['buttons']['yes']:
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == state.selected_ficha.id).first()
                    if ficha:
                        ficha.is_active = False
                        session.commit()
                        
                        state.deselect()
                        prefs = ft.SharedPreferences()
                        await prefs.remove("selected_ficha")
                        await load_fichas()
                        
                        # Limpiar editor
                        selected_card_title.value = "Selecciona una tarjeta"
                        markdown_editor.value = ""
                        markdown_preview.value = ""
                        
                        page.show_dialog(ft.SnackBar(
                            content=ft.Text(t['delete']['success']),
                            bgcolor=ft.Colors.GREEN_400,
                            duration=2000
                        ))
                except Exception as e:
                    session.rollback()
                    print(f"Error eliminando ficha: {str(e)}")
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text(t['delete']['error']),
                        bgcolor=ft.Colors.RED_400,
                        action="Ok",
                        duration=2000
                    ))
                finally:
                    session.close()
            
            dialog.open = False
            page.update()
        
        dialog = ft.AlertDialog(
            modal=True,
            title=ft.Text(t['delete']['confirm_title']),
            content=ft.Text(t['delete']['confirm_message']),
            actions=[
                ft.TextButton(t['buttons']['yes'], on_click=confirm_delete),
                ft.TextButton(t['buttons']['no'], on_click=lambda e: (setattr(dialog, 'open', False), page.update())),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )
        
        page.overlay.append(dialog)
        dialog.open = True
        page.update()
    
    # Asignar eventos
    markdown_editor.on_change = on_editor_change
    search_field.on_change = on_search_change
    
    # ==================== LAYOUT ====================
    
    sidebar = create_sidebar(
        search_field=search_field,
        cards_listview=cards_listview,
        card_counter=card_counter,
        new_card_callback=new_card_handler,
        recycle_bin_callback=recycle_bin_handler,
        settings_callback=settings_handler,
        logout_callback=logout_handler
    )
    
    main_panel = ft.Container(
        content=ft.Column(
            [
                header_container,
                ft.Divider(height=1, color=theme_manager.divider_color),
                ft.Container(
                    content=ft.Column(
                        [
                            tabs_row, # De create_custom_tabs
                            ft.Divider(height=1, color=ft.Colors.TRANSPARENT),
                            markdown_toolbar,
                            ft.Divider(height=1, color=ft.Colors.TRANSPARENT),
                            editor_container,
                            preview_container,
                        ],
                        spacing=0, expand=True,
                    ),
                    expand=True,
                ),
            ],
            spacing=0,
        ),
        expand=True,
        bgcolor=theme_manager.bg,
    )
    
    # Layout principal
    main_layout = ft.Row([sidebar, main_panel], spacing=0, expand=True)
    
    # Vista final con Stack para modales
    main_view = ft.Stack(
        [
            main_layout,
            modal_overlay,
        ],
        expand=True,
    )
    
    # ==================== LIFECYCLE ====================
    
    def on_view_mount():
        asyncio.create_task(load_fichas())
    
    async def on_view_unmount():
        if state.has_unsaved_changes:
            await save_current_ficha()
        await state.cleanup()
    
    main_view.did_mount = on_view_mount
    main_view.will_unmount = on_view_unmount
    
    # Asignar handler de eliminación
    page.delete_ficha = delete_ficha_handler
    page.open_settings = settings_handler
    
    return main_view
