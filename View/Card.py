import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha
import threading
import time
from typing import Optional
from config.config import Config

def card_view(page: Page):
    # Atributos
    selected_ficha = None  # Variable para mantener la ficha seleccionada
    save_thread: Optional[threading.Thread] = None
    should_save = False
    
    fichas_list = ft.ListView(
        expand=1,
        spacing=10,
        padding=20,
        height=500
    )
    
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

    description_text = ft.TextField(
        multiline=True,
        min_lines=16,
        max_lines=16,
        read_only=True,
        border_color=ft.Colors.BLUE_200,
        bgcolor=ft.Colors.WHITE10,
        expand=True
    )

    # Agregar el control deslizante y su función manejadora
    def toggle_readonly(e):
        """Maneja el cambio del switch de modo lectura"""
        nonlocal save_thread, should_save
        description_text.read_only = not description_text.read_only
        
        if not description_text.read_only:  # Si se activa el modo edición
            should_save = True
            save_thread = threading.Thread(target=auto_save)
            save_thread.daemon = True  # El hilo se cerrará cuando el programa principal termine
            save_thread.start()
        else:  # Si se desactiva el modo edición
            should_save = False
            if save_thread:
                save_thread.join(timeout=1.1)  # Esperar a que termine el último guardado
                save_thread = None
        
        description_text.update()
        
    edit_switch = ft.Row(
        controls=[
            ft.Text(t['read_mode']),
            ft.Switch(
                value=False,
                on_change=toggle_readonly,
                active_color=ft.Colors.BLUE,
                disabled=True
            )
        ],
        alignment=ft.MainAxisAlignment.END
    )

    # Agregar el campo de búsqueda
    def filter_fichas(e):
        """Filtra las fichas basado en el texto de búsqueda"""
        search_text = e.control.value.lower().strip() if e.control.value else ""
        session = get_session()
        try:
            user_id = page.client_storage.get("user_id")
            # Obtener todas las fichas del usuario
            fichas = session.query(Ficha).filter(Ficha.usuario_id == user_id).all()
            
            # Si no hay texto de búsqueda, mostrar todas las fichas
            if not search_text:
                filtered_fichas = fichas
            else:
                # Filtrar las fichas que contienen el texto de búsqueda en el título
                filtered_fichas = [
                    ficha for ficha in fichas 
                    if search_text in ficha.title.lower()
                ]
            
            # Actualizar la lista con las fichas filtradas
            fichas_list.controls = [
                ft.ListTile(
                    leading=ft.Icon(ft.Icons.DESCRIPTION),
                    title=ft.Text(ficha.title),
                    on_click=lambda e, ficha=ficha: select_ficha(ficha)
                ) for ficha in filtered_fichas
            ]
            fichas_list.update()
            page.update()
        except Exception as e:
            print(f"Error filtrando fichas: {str(e)}")
        finally:
            session.close()

    # Actualizar el campo de búsqueda para usar la función de filtrado
    search_field = ft.TextField(
        label=t['search']['label'],
        on_change=filter_fichas,  # Conectar la función de filtrado
        expand=True,
        border_color=ft.Colors.BLUE_200,
        hint_text=t['search']['hint'],  # Texto de ayuda
    )

    def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        nonlocal selected_ficha, should_save
        
        # Guardar la ficha seleccionada en el almacenamiento del cliente
        page.client_storage.set("selected_ficha", {
            "id": ficha.id,
            "title": ficha.title,
            "descripcion": ficha.descripcion
        })
        
        # Habilitar el switch cuando se selecciona una ficha
        edit_switch.controls[1].disabled = False
        
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
                edit_switch.controls[1].value = False
                should_save = False
                print(f"✅ Ficha {ficha_actualizada.id} cargada con éxito")
        except Exception as e:
            print(f"❌ Error cargando ficha: {str(e)}")
        finally:
            session.close()
            
        page.update()

    def delete_ficha_handler(e=None):
        nonlocal selected_ficha
        
        if not selected_ficha:
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text(t['delete']['no_selection']),
                    bgcolor=ft.Colors.RED_400,
                    action=t['buttons']['ok']
                )
            )
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
                        
                        page.show_snack_bar(
                            ft.SnackBar(
                                content=ft.Text(t['delete']['success']),
                                bgcolor=ft.Colors.GREEN_400,
                                action=t['buttons']['ok']
                            )
                        )
                    
                except Exception as e:
                    session.rollback()
                    print(f"Error desactivando ficha: {str(e)}")
                    page.show_snack_bar(
                        ft.SnackBar(
                            content=ft.Text(t['delete']['error']),
                            bgcolor=ft.Colors.RED_400,
                            action=t['buttons']['ok']
                        )
                    )
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
            fichas = session.query(Ficha).filter(
                Ficha.usuario_id == user_id,
                Ficha.is_active == True
            ).all()
            
            # Crear los controles antes de asignarlos
            controls = [
                ft.ListTile(
                    leading=ft.Icon(ft.Icons.DESCRIPTION),
                    title=ft.Text(ficha.title),
                    on_click=lambda e, ficha=ficha: select_ficha(ficha)
                ) for ficha in fichas
            ]
            
            # Asignar los controles al ListView
            fichas_list.controls = controls
            page.update()
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    def auto_save():
        """Función que se ejecuta en un hilo separado para guardar periódicamente"""
        nonlocal should_save, selected_ficha
        while should_save:
            if selected_ficha and description_text.value != selected_ficha.descripcion:
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        ficha.descripcion = description_text.value
                        session.commit()
                        print("Guardado automático realizado")
                except Exception as e:
                    print(f"Error en guardado automático: {str(e)}")
                finally:
                    session.close()
            time.sleep(1)  # Esperar 1 segundo

    def clear_selection():
        """Limpia la selección actual"""
        nonlocal selected_ficha, should_save
        
        page.client_storage.remove("selected_ficha")
        selected_ficha = None
        title_label.value = ""
        description_text.value = ""
        description_text.read_only = True
        edit_switch.controls[1].disabled = True
        edit_switch.controls[1].value = False
        should_save = False
        page.update()

    # Panel derecho completo
    right_panel = ft.Container(
        content=ft.Column(
            controls=[
                title_label,
                edit_switch,  # Agregar el switch aquí
                ft.Divider(height=20, color=ft.Colors.TRANSPARENT),
                description_text
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
    
    main_view.did_mount = on_view_mount
    return main_view