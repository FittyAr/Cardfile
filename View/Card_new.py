import flet as ft
from data.database.connection import get_session
from data.models.ficha import Ficha
from datetime import datetime
from config.config import Config
import asyncio

async def card_view(page: ft.Page):
    """Vista moderna de tarjetas con diseño profesional tipo dashboard"""
    config = Config()
    t = config.translations['card']
    
    # Variables de estado
    selected_ficha = None
    fichas_list = []
    search_query = ""
    debounce_task = None
    autosave_task = None
    last_saved_value = ""
    has_unsaved_changes = False
    
    # ==================== COMPONENTES DE UI ====================
    
    # Barra de búsqueda moderna
    search_field = ft.TextField(
        hint_text="🔍 " + t['search']['hint'],
        border_radius=12,
        filled=True,
        bgcolor=ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE),
        border_color=ft.Colors.TRANSPARENT,
        focused_border_color=ft.Colors.BLUE_400,
        content_padding=ft.padding.symmetric(horizontal=20, vertical=15),
        text_size=14,
        expand=True,
    )
    
    # Contador de tarjetas
    card_counter = ft.Text(
        "0 tarjetas",
        size=13,
        color=ft.Colors.with_opacity(0.6, ft.Colors.ON_SURFACE),
        weight=ft.FontWeight.W_500,
    )
    
    # Lista de tarjetas (sidebar izquierdo)
    cards_listview = ft.ListView(
        spacing=8,
        padding=ft.padding.all(12),
        expand=True,
    )
    
    # Editor de markdown
    markdown_editor = ft.TextField(
        multiline=True,
        min_lines=20,
        max_lines=None,
        border_radius=12,
        filled=True,
        bgcolor=ft.Colors.with_opacity(0.03, ft.Colors.ON_SURFACE),
        border_color=ft.Colors.TRANSPARENT,
        focused_border_color=ft.Colors.BLUE_400,
        content_padding=ft.padding.all(20),
        text_size=14,
        hint_text="Escribe aquí tu contenido en Markdown...",
        expand=True,
    )
    
    # Preview de markdown
    markdown_preview = ft.Markdown(
        "",
        selectable=True,
        extension_set=ft.MarkdownExtensionSet.GITHUB_WEB,
        on_tap_link=lambda e: page.launch_url(e.data),
        expand=True,
    )
    
    # Indicador de guardado
    save_indicator = ft.Row(
        [
            ft.Icon(ft.Icons.CHECK_CIRCLE, color=ft.Colors.GREEN_400, size=16),
            ft.Text("Guardado", size=12, color=ft.Colors.GREEN_400),
        ],
        spacing=6,
        visible=False,
    )
    
    # Título de la tarjeta seleccionada
    selected_card_title = ft.Text(
        "Selecciona una tarjeta",
        size=24,
        weight=ft.FontWeight.BOLD,
        color=ft.Colors.ON_SURFACE,
    )
    
    # Tabs para Editor/Preview
    editor_tabs = ft.Tabs(
        selected_index=0,
        animation_duration=300,
        tabs=[
            ft.Tab(
                text="✏️ Editor",
                content=ft.Container(
                    content=markdown_editor,
                    padding=ft.padding.all(0),
                    expand=True,
                ),
            ),
            ft.Tab(
                text="👁️ Vista Previa",
                content=ft.Container(
                    content=ft.Column(
                        [
                            markdown_preview,
                        ],
                        scroll=ft.ScrollMode.AUTO,
                        expand=True,
                    ),
                    padding=ft.padding.all(20),
                    expand=True,
                ),
            ),
        ],
        expand=True,
    )
    
    # ==================== FUNCIONES ====================
    
    async def load_fichas(search_text=""):
        """Carga las fichas del usuario"""
        nonlocal fichas_list
        session = get_session()
        try:
            user_id = await page.shared_preferences.get("user_id")
            user_id = int(user_id) if user_id else None
            q = session.query(Ficha).filter(Ficha.usuario_id == user_id, Ficha.is_active == True)
            if search_text:
                q = q.filter(Ficha.title.ilike(f"%{search_text}%"))
            fichas = q.all()
            fichas_list = fichas
            render_fichas_list(fichas)
            
            # Actualizar contador
            card_counter.value = f"{len(fichas)} tarjeta{'s' if len(fichas) != 1 else ''}"
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
                            color=ft.Colors.ON_SURFACE,
                            max_lines=1,
                            overflow=ft.TextOverflow.ELLIPSIS,
                        ),
                        ft.Text(
                            f"Actualizado: {ficha.updated_at.strftime('%d/%m/%Y')}" if ficha.updated_at else "Sin fecha",
                            size=11,
                            color=ft.Colors.with_opacity(0.5, ft.Colors.ON_SURFACE),
                        ),
                    ],
                    spacing=4,
                ),
                padding=ft.padding.all(16),
                border_radius=10,
                bgcolor=ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE),
                ink=True,
                on_click=lambda e, f=ficha: asyncio.create_task(select_ficha(f)),
                animate=ft.animation.Animation(200, ft.AnimationCurve.EASE_OUT),
            )
            cards_listview.controls.append(card_item)
        
        page.update()
    
    async def select_ficha(ficha):
        """Selecciona una tarjeta"""
        nonlocal selected_ficha, last_saved_value, has_unsaved_changes
        
        # Guardar cambios pendientes de la tarjeta anterior
        if selected_ficha and has_unsaved_changes:
            await save_current_ficha()
        
        selected_ficha = ficha
        
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
        last_saved_value = ficha.descripcion or ""
        has_unsaved_changes = False
        
        # Resaltar tarjeta seleccionada
        for i, control in enumerate(cards_listview.controls):
            if i < len(fichas_list) and fichas_list[i].id == ficha.id:
                control.bgcolor = ft.Colors.BLUE_400
                control.content.controls[0].color = ft.Colors.WHITE
                control.content.controls[1].color = ft.Colors.with_opacity(0.8, ft.Colors.WHITE)
            else:
                control.bgcolor = ft.Colors.with_opacity(0.05, ft.Colors.ON_SURFACE)
                control.content.controls[0].color = ft.Colors.ON_SURFACE
                control.content.controls[1].color = ft.Colors.with_opacity(0.5, ft.Colors.ON_SURFACE)
        
        page.update()
    
    async def save_current_ficha():
        """Guarda la tarjeta actual"""
        nonlocal has_unsaved_changes, last_saved_value
        
        if not selected_ficha:
            return
        
        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
            if ficha:
                ficha.descripcion = markdown_editor.value
                session.commit()
                last_saved_value = markdown_editor.value
                has_unsaved_changes = False
                
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
    
    async def on_editor_change(e):
        """Maneja cambios en el editor"""
        nonlocal has_unsaved_changes, debounce_task
        
        has_unsaved_changes = True
        markdown_preview.value = markdown_editor.value
        
        # Debounce para autosave
        if debounce_task:
            debounce_task.cancel()
        
        debounce_task = asyncio.create_task(debounced_save())
    
    async def debounced_save():
        """Guarda después de 2 segundos de inactividad"""
        await asyncio.sleep(2)
        await save_current_ficha()
    
    async def on_search_change(e):
        """Maneja cambios en la búsqueda"""
        await load_fichas(search_field.value)
    
    async def delete_ficha_handler(e=None):
        """Elimina la tarjeta seleccionada"""
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
            button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
            if button_text == t['buttons']['yes']:
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        ficha.is_active = False
                        session.commit()
                        
                        selected_ficha = None
                        await page.shared_preferences.remove("selected_ficha")
                        await load_fichas()
                        
                        # Limpiar editor
                        selected_card_title.value = "Selecciona una tarjeta"
                        markdown_editor.value = ""
                        markdown_preview.value = ""
                        
                        page.show_dialog(ft.SnackBar(
                            content=ft.Text(t['delete']['success']),
                            bgcolor=ft.Colors.GREEN_400,
                            action="Ok"
                        ))
                        page.update()
                except Exception as e:
                    session.rollback()
                    print(f"Error eliminando ficha: {str(e)}")
                    page.show_dialog(ft.SnackBar(
                        content=ft.Text(t['delete']['error']),
                        bgcolor=ft.Colors.RED_400,
                        action="Ok"
                    ))
                    page.update()
                finally:
                    session.close()
            
            page.close(dialog)
        
        dialog = ft.AlertDialog(
            modal=True,
            title=ft.Text(t['delete']['confirm_title']),
            content=ft.Text(t['delete']['confirm_message']),
            actions=[
                ft.TextButton(t['buttons']['yes'], on_click=confirm_delete),
                ft.TextButton(t['buttons']['no'], on_click=lambda e: page.close(dialog)),
            ],
            actions_alignment=ft.MainAxisAlignment.END,
        )
        
        page.open(dialog)
    
    # Asignar eventos
    markdown_editor.on_change = on_editor_change
    search_field.on_change = on_search_change
    
    # ==================== LAYOUT ====================
    
    # Sidebar izquierdo (lista de tarjetas)
    sidebar = ft.Container(
        content=ft.Column(
            [
                # Header del sidebar
                ft.Container(
                    content=ft.Column(
                        [
                            ft.Text(
                                "📚 Mis Tarjetas",
                                size=18,
                                weight=ft.FontWeight.BOLD,
                                color=ft.Colors.ON_SURFACE,
                            ),
                            card_counter,
                        ],
                        spacing=4,
                    ),
                    padding=ft.padding.all(16),
                ),
                # Búsqueda
                ft.Container(
                    content=search_field,
                    padding=ft.padding.symmetric(horizontal=12),
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                # Lista de tarjetas
                ft.Container(
                    content=cards_listview,
                    expand=True,
                ),
            ],
            spacing=12,
        ),
        width=320,
        bgcolor=ft.Colors.with_opacity(0.02, ft.Colors.ON_SURFACE),
        border=ft.border.only(right=ft.BorderSide(1, ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE))),
        expand=True,
    )
    
    # Panel principal (editor)
    main_panel = ft.Container(
        content=ft.Column(
            [
                # Header del panel principal
                ft.Container(
                    content=ft.Row(
                        [
                            selected_card_title,
                            save_indicator,
                        ],
                        alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                    ),
                    padding=ft.padding.all(20),
                ),
                ft.Divider(height=1, color=ft.Colors.with_opacity(0.1, ft.Colors.ON_SURFACE)),
                # Editor con tabs
                ft.Container(
                    content=editor_tabs,
                    expand=True,
                ),
            ],
            spacing=0,
        ),
        expand=True,
    )
    
    # Layout principal
    main_view = ft.Row(
        [
            sidebar,
            main_panel,
        ],
        spacing=0,
        expand=True,
    )
    
    # ==================== LIFECYCLE ====================
    
    def on_view_mount():
        asyncio.create_task(load_fichas())
    
    async def on_view_unmount():
        nonlocal debounce_task, autosave_task
        try:
            if debounce_task:
                debounce_task.cancel()
            if has_unsaved_changes:
                await save_current_ficha()
        finally:
            if autosave_task:
                autosave_task.cancel()
    
    main_view.did_mount = on_view_mount
    main_view.will_unmount = on_view_unmount
    
    # Asignar handler de eliminación
    page.delete_ficha = delete_ficha_handler
    
    return main_view
