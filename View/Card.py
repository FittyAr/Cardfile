import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config
import asyncio
from theme.manager import ThemeManager

theme_manager = ThemeManager()

# Importar componentes modularizados
from View.components.markdown_editor import (
    create_markdown_editor,
    create_markdown_toolbar,
    create_markdown_preview
)
from View.components.card_ui import (
    create_sidebar,
    create_card_header,
    create_custom_tabs,
    create_save_indicator,
    create_card_counter,
    create_cards_listview,
    create_search_field
)
from View.components.card_state import CardState

# Importar componentes de modales
from View.NewCard import new_card_modal
from View.EditCard import edit_card_modal
from View.Recycle import recycle_modal
from View.Settings import settings_modal
from theme.colors import ThemeColors

async def card_view(page: ft.Page):
    """Vista moderna de tarjetas con diseño profesional tipo dashboard"""
    config = Config()
    t = config.translations['card']
    
    # Aplicar modo oscuro/claro según el tema
    page.theme_mode = ft.ThemeMode.DARK if theme_manager.is_dark else ft.ThemeMode.LIGHT
    
    # Estado centralizado
    state = CardState()
    search_query = ""
    
    # ==================== COMPONENTES DE UI ====================
    
    # --- Modales ---
    async def hide_modal():
        modal_overlay.visible = False
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

    async def change_theme_handler(theme_name):
        """Handler para cambiar el tema de la aplicación"""
        theme_manager.set_theme(theme_name)
        # Recargar los controles de la vista actual
        new_content = await card_view(page)
        if page.views:
            page.views[-1].controls = [new_content]
        page.update()

    async def recycle_bin_handler(e):
        modal_content = await recycle_modal(page, on_close=hide_modal, on_success=on_modal_success)
        modal_overlay.content = modal_content
        modal_overlay.visible = True
        page.update()

    from View.components.auth_manager import AuthManager
    auth_manager = AuthManager(page)
    if auth_manager.require_login and not await auth_manager.is_authenticated():
        await page.push_route("/Login")
        return ft.Container()
    
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

    header_container, edit_header_btn, delete_header_btn = create_card_header(
        selected_card_title, 
        save_indicator,
        edit_callback=edit_card_handler,
        delete_callback=delete_ficha_handler
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
        editor_enabled = state.editor_should_be_enabled()
        
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
            selected_card_title.value = state.selected_ficha.title
        
        # Deshabilitar tabs y botones de acción si no hay selección
        editor_btn.disabled = not editor_enabled
        preview_btn.disabled = not editor_enabled
        edit_header_btn.disabled = not editor_enabled
        delete_header_btn.disabled = not editor_enabled
        
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
    
    def render_fichas_list(fichas):
        """Renderiza la lista de tarjetas en el sidebar"""
        cards_listview.controls.clear()
        
        for ficha in fichas:
            # Tarjeta individual con diseño moderno
            title_text = ft.Text(
                ficha.title,
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
            card_item = ft.Container(
                content=ft.Row(
                    [
                        ft.Column(
                            [
                                title_text,
                                date_text,
                            ],
                            spacing=theme_manager.space_4,
                            expand=True,
                        )
                    ],
                    expand=True,
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
    
    async def select_ficha(ficha):
        """Selecciona una tarjeta"""
        # Guardar cambios pendientes de la tarjeta anterior
        if state.selected_ficha and state.has_unsaved_changes:
            await save_current_ficha()
        
        state.select_ficha(ficha)  # Usar método del state
        
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
        selected_card_title.value = ficha.title
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
    
    # --- Theme Switcher ---
    theme_selector = ft.Row(
        [
            ft.Container(
                content=ft.IconButton(
                    icon=ft.Icons.CIRCLE,
                    icon_color=color_palette["primary"],
                    tooltip=color_palette["name"],
                    on_click=lambda _, n=name: asyncio.create_task(change_theme_handler(n)),
                    icon_size=theme_manager.icon_size_md,
                    style=ft.ButtonStyle(
                        padding=0,
                    )
                ),
                border=ft.Border.all(2, theme_manager.primary) if theme_manager.current_theme_name == name else None,
                border_radius=theme_manager.radius_round,
                padding=theme_manager.space_4,
            )
            for name, color_palette in ThemeColors.THEMES.items()
        ],
        spacing=theme_manager.space_8,
        alignment=ft.MainAxisAlignment.CENTER,
    )

    sidebar = create_sidebar(
        search_field=search_field,
        cards_listview=cards_listview,
        card_counter=card_counter,
        new_card_callback=new_card_handler,
        recycle_bin_callback=recycle_bin_handler,
        settings_callback=settings_handler,
        logout_callback=logout_handler
    )
    
    # Inyectar el selector de temas en el sidebar (esto requiere modificar create_sidebar o hacerlo aquí)
    # Por simplicidad y minimalismo, lo añadiremos al final del sidebar_content
    sidebar.content.controls.insert(-1, ft.Container(
        content=ft.Column([
            ft.Text("Tema", size=theme_manager.text_size_sm, weight=ft.FontWeight.BOLD, color=theme_manager.subtext),
            theme_selector,
        ], spacing=theme_manager.space_8, horizontal_alignment=ft.CrossAxisAlignment.CENTER),
        padding=ft.Padding.symmetric(horizontal=theme_manager.space_20, vertical=theme_manager.space_12)
    ))
    
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
        state.cleanup()
    
    main_view.did_mount = on_view_mount
    main_view.will_unmount = on_view_unmount
    
    # Asignar handler de eliminación
    page.delete_ficha = delete_ficha_handler
    page.open_settings = settings_handler
    
    return main_view
