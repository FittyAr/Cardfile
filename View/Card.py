import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha

def card_view(page: Page):
    # Atributos
    selected_ficha = None  # Variable para mantener la ficha seleccionada
    
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
        min_lines=10,
        max_lines=20,
        read_only=True,
        border_color=ft.colors.BLUE_200,
        bgcolor=ft.colors.WHITE10,
        expand=True
    )

    # Agregar el campo de búsqueda
    search_field = ft.TextField(
        label="Buscar",
        on_change=lambda e: print(e.control.value),
        expand=True,
        border_color=ft.colors.BLUE_200,
    )

    def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        nonlocal selected_ficha  # Acceder a la variable externa
        selected_ficha = ficha
        title_label.value = ficha.title
        description_text.value = ficha.descripcion
        page.update()

    def delete_ficha_handler(e=None):
        nonlocal selected_ficha  # Mover la declaración nonlocal al principio de la función
        
        if not selected_ficha:
            page.show_snack_bar(
                ft.SnackBar(
                    content=ft.Text("Por favor seleccione una ficha para eliminar"),
                    bgcolor=ft.colors.RED_400,
                    action="Ok"
                )
            )
            return

        session = get_session()
        try:
            ficha = session.query(Ficha).filter(Ficha.id == selected_ficha.id).first()
            if ficha:
                session.delete(ficha)
                session.commit()
                
                # Limpiar la selección
                selected_ficha = None
                title_label.value = "Seleccione una ficha"
                description_text.value = ""
                
                # Recargar la lista de fichas
                load_fichas()
                
                page.show_snack_bar(
                    ft.SnackBar(
                        content=ft.Text("Ficha eliminada exitosamente"),
                        bgcolor=ft.colors.GREEN_400,
                        action="Ok"
                    )
                )
                page.update()
            
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
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    # Panel derecho completo
    right_panel = ft.Container(
        content=ft.Column(
            controls=[
                title_label,
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