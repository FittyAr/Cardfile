import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha

def card_view(page: Page):
    # Atributos
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
        width=400
    )

    def select_ficha(ficha):
        """Maneja la selección de una ficha"""
        title_label.value = ficha.title
        description_text.value = ficha.descripcion
        page.update()

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
            horizontal_alignment=ft.CrossAxisAlignment.CENTER
        ),
        padding=20
    )

    # Contenedor principal
    main_view = ft.Container(
        content=ft.Row(
            controls=[
                ft.Container(
                    content=ft.Column(
                        controls=[
                            ft.Text("Mis Fichas", size=20, weight=ft.FontWeight.BOLD),
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
                    padding=10
                )
            ],
            alignment=ft.MainAxisAlignment.CENTER,
            vertical_alignment=ft.CrossAxisAlignment.START
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