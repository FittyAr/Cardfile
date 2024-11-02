import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha
import threading
import time
from typing import Optional

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
    
    title_label = ft.Text(
        value="Seleccione una ficha",
        size=24,
        weight=ft.FontWeight.BOLD,
        color=ft.colors.BLUE
    )

    description_text = ft.TextField(
        multiline=True,
        min_lines=16,
        max_lines=16,
        read_only=True,
        border_color=ft.colors.BLUE_200,
        bgcolor=ft.colors.WHITE10,
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
            ft.Text("Modo lectura:"),
            ft.Switch(
                value=False,
                on_change=toggle_readonly,
                active_color=ft.colors.BLUE,
                disabled=True  # Inicialmente deshabilitado
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
                    leading=ft.Icon(ft.icons.DESCRIPTION),
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
        label="Buscar",
        on_change=filter_fichas,  # Conectar la función de filtrado
        expand=True,
        border_color=ft.colors.BLUE_200,
        hint_text="Escriba para buscar...",  # Texto de ayuda
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
        
        # Habilitar el botón de editar
        for control in page.views[-1].floating_action_button.controls:
            if getattr(control, 'data', None) == "edit_button":
                control.disabled = False
                break

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
                description_text.read_only = True  # Asegurar que comience en modo lectura
                edit_switch.controls[1].value = False  # Resetear el switch a modo lectura
                should_save = False  # Detener el guardado automático si estaba activo
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
                    content=ft.Text("Por favor seleccione una ficha para eliminar"),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
            return

        def confirm_delete(e):
            nonlocal selected_ficha
            if e.control.text == "Sí":
                session = get_session()
                try:
                    ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
                    if ficha:
                        session.delete(ficha)
                        session.commit()
                        
                        # Limpiar la selección y deshabilitar controles
                        clear_selection()
                        
                        # Recargar la lista de fichas
                        load_fichas()
                        
                        page.show_snack_bar(
                            ft.SnackBar(
                                content=ft.Text("Ficha eliminada exitosamente"),
                                bgcolor=ft.colors.GREEN_400,
                                action="Ok"
                            )
                        )
                    
                except Exception as e:
                    session.rollback()
                    print(f"Error eliminando ficha: {str(e)}")
                    page.show_snack_bar(
                        ft.SnackBar(
                            content=ft.Text("Error al eliminar la ficha"),
                            bgcolor=ft.colors.RED_400,
                            action="Ok"
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
            title=ft.Text("Confirmar eliminación"),
            content=ft.Text("¿Está seguro que desea eliminar esta ficha?"),
            actions=[
                ft.TextButton("No", on_click=confirm_delete),
                ft.TextButton("Sí", on_click=confirm_delete),
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
        """Carga las fichas del usuario actual desde la base de datos"""
        session = get_session()
        try:
            user_id = page.client_storage.get("user_id")
            fichas = session.query(Ficha).filter(Ficha.usuario_id == user_id).all()
            
            fichas_list.controls = [
                ft.ListTile(
                    leading=ft.Icon(ft.icons.DESCRIPTION),
                    title=ft.Text(ficha.title),
                    on_click=lambda e, ficha=ficha: select_ficha(ficha)
                ) for ficha in fichas
            ]
            fichas_list.update()
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
        
        # Limpiar la ficha seleccionada del almacenamiento del cliente
        page.client_storage.remove("selected_ficha")
        
        selected_ficha = None
        title_label.value = ""
        description_text.value = ""
        description_text.read_only = True
        edit_switch.controls[1].disabled = True
        edit_switch.controls[1].value = False
        should_save = False
        
        # Deshabilitar el botón de editar
        for control in page.views[-1].floating_action_button.controls:
            if getattr(control, 'data', None) == "edit_button":
                control.disabled = True
                break
                
        page.update()

    # Panel derecho completo
    right_panel = ft.Container(
        content=ft.Column(
            controls=[
                title_label,
                edit_switch,  # Agregar el switch aquí
                ft.Divider(height=20, color=ft.colors.TRANSPARENT),
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
                            search_field,  # Solo el campo de búsqueda
                            ft.Divider(height=10, color=ft.colors.TRANSPARENT),  # Espacio entre búsqueda y lista
                            fichas_list
                        ]
                    ),
                    width=300,
                    border=ft.border.all(2, ft.colors.BLUE_200),
                    border_radius=10,
                    padding=10
                ),
                ft.VerticalDivider(width=20, color=ft.colors.TRANSPARENT),
                ft.Container(
                    content=right_panel,
                    border=ft.border.all(2, ft.colors.BLUE_200),
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

    # Configurar el botón flotante
    page.floating_action_button = ft.FloatingActionButton(
        icon=ft.icons.ADD,
        on_click=lambda _: page.go("/newCard"),
        tooltip="Agregar nueva tarjeta",
        bgcolor=ft.colors.BLUE,
    )

    # Cargar fichas al iniciar
    load_fichas()
    
    return main_view