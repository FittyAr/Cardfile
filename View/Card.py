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
        markdown_preview.visible = False
        editor_container.visible = True
        editor_btn.bgcolor = theme_manager.primary
        editor_btn.content.color = ft.Colors.WHITE
        preview_btn.bgcolor = ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE)
        preview_btn.content.color = ft.Colors.ON_SURFACE
        page.update()

    def on_preview_tab_click(e):
        editor_container.visible = False
        preview_container.visible = True
        markdown_toolbar.visible = False
        markdown_preview.value = markdown_editor.value
        preview_btn.bgcolor = theme_manager.primary
        preview_btn.content.color = ft.Colors.WHITE
        editor_btn.bgcolor = ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE)
        editor_btn.content.color = ft.Colors.ON_SURFACE
        page.update()

    async def edit_card_handler(e):
        """Handler para el botón de editar título"""
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

    async def logout_handler(e):
        """Handler para el botón de cerrar sesión"""
        await page.shared_preferences.remove("selected_ficha")
        await page.push_route("/Login")

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
        size=24, weight=ft.FontWeight.BOLD, color=theme_manager.text
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

    editor_container = ft.Container(content=markdown_editor, expand=True, visible=True)
    preview_container = ft.Container(
        content=ft.Column([markdown_preview], scroll=ft.ScrollMode.AUTO, expand=True),
        padding=ft.Padding.all(20), expand=True, visible=False
    )
    
    # --- Overlay Modal con Blur ---
    modal_overlay = ft.Container(
        content=None,
        bgcolor=ft.Colors.with_opacity(0.2, ft.Colors.BLACK),
        blur=ft.Blur(10, 10),
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
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            q = session.query(Ficha).filter(Ficha.usuario_id == user_id, Ficha.is_active == True)
            if search_text:
                q = q.filter(Ficha.title.ilike(f"%{search_text}%"))
            fichas = q.all()
            state.fichas_list = fichas  # Usar state object
            render_fichas_list(fichas)
            
            # Actualizar contador
            card_counter.value = f"{len(fichas)} tarjeta{'s' if len(fichas) != 1 else ''}"
            
            # Intentar restaurar selección previa
            selected_ficha_data = await page.shared_preferences.get("selected_ficha")
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
            card_item = ft.Container(
                content=ft.Column(
                    [
                        ft.Text(
                            ficha.title,
                            size=14,
                            weight=ft.FontWeight.W_600,
                            color=theme_manager.text,
                            max_lines=1,
                            overflow=ft.TextOverflow.ELLIPSIS,
                        ),
                        ft.Text(
                            f"Actualizado: {ficha.updated_at.strftime('%d/%m/%Y')}" if ficha.updated_at else "Sin fecha",
                            size=11,
                            color=theme_manager.subtext,
                        ),
                    ],
                    spacing=4,
                ),
                padding=ft.Padding.all(16),
                border_radius=10,
                bgcolor=ft.Colors.with_opacity(0.1, theme_manager.text),
                ink=True,
                on_click=lambda e, f=ficha: asyncio.create_task(select_ficha(f)),
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
        await page.shared_preferences.set("selected_ficha", ficha_data)
        
        # Actualizar UI
        selected_card_title.value = ficha.title
        markdown_editor.value = ficha.descripcion or ""
        markdown_preview.value = ficha.descripcion or ""
        
        # Resaltar tarjeta seleccionada
        for i, control in enumerate(cards_listview.controls):
            if i < len(state.fichas_list) and state.fichas_list[i].id == ficha.id:
                control.bgcolor = theme_manager.primary
                control.content.controls[0].color = ft.Colors.WHITE
                control.content.controls[1].color = ft.Colors.with_opacity(0.8, ft.Colors.WHITE)
            else:
                control.bgcolor = ft.Colors.with_opacity(0.1, theme_manager.text)
                control.content.controls[0].color = theme_manager.text
                control.content.controls[1].color = theme_manager.subtext
        
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
                action="Ok"
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
                        await page.shared_preferences.remove("selected_ficha")
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
                        action="Ok"
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
                    icon_size=20,
                    style=ft.ButtonStyle(
                        padding=0,
                    )
                ),
                border=ft.border.all(2, theme_manager.primary) if theme_manager.current_theme_name == name else None,
                border_radius=50,
                padding=2,
            )
            for name, color_palette in ThemeColors.THEMES.items()
        ],
        spacing=5,
        alignment=ft.MainAxisAlignment.CENTER,
    )

    sidebar = create_sidebar(
        search_field=search_field,
        cards_listview=cards_listview,
        card_counter=card_counter,
        new_card_callback=new_card_handler,
        recycle_bin_callback=recycle_bin_handler,
        logout_callback=logout_handler
    )
    
    # Inyectar el selector de temas en el sidebar (esto requiere modificar create_sidebar o hacerlo aquí)
    # Por simplicidad y minimalismo, lo añadiremos al final del sidebar_content
    sidebar.content.controls.insert(-1, ft.Container(
        content=ft.Column([
            ft.Text("Tema", size=12, weight=ft.FontWeight.BOLD, color=theme_manager.subtext),
            theme_selector,
        ], spacing=5, horizontal_alignment=ft.CrossAxisAlignment.CENTER),
        padding=ft.Padding.symmetric(horizontal=20, vertical=10)
    ))
    
    main_panel = ft.Container(
        content=ft.Column(
            [
                header_container,
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, theme_manager.text)),
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
    
    return main_view
