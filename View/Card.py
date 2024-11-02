import flet as ft
from flet import Page
from data.database.connection import get_session
from data.models.ficha import Ficha




class CardView(ft.UserControl):
    def __init__(self, page: Page):
        super().__init__()
        self.page = page
        self.selected_ficha = None
        self.fichas_list = None
        self.title_label = None
        self.description_text = None

    def load_fichas(self):
        """Carga las fichas del usuario actual desde la base de datos"""
        session = get_session()
        try:
            user_id = self.page.session.get("user_id")
            fichas = session.query(Ficha).filter(Ficha.usuario_id == user_id).all()
            
            # Actualizar ListView con las fichas
            self.fichas_list.controls = [
                ft.ListTile(
                    leading=ft.Icon(ft.icons.DESCRIPTION),
                    title=ft.Text(ficha.title),
                    on_click=lambda e, ficha=ficha: self.select_ficha(ficha)
                ) for ficha in fichas
            ]
            self.fichas_list.update()
        except Exception as e:
            print(f"Error cargando fichas: {str(e)}")
        finally:
            session.close()

    def select_ficha(self, ficha):
        """Maneja la selección de una ficha"""
        self.selected_ficha = ficha
        self.title_label.value = ficha.title
        self.description_text.value = ficha.descripcion
        self.update()

    def build(self):
        # Panel izquierdo con ListView
        self.fichas_list = ft.ListView(
            expand=1,
            spacing=10,
            padding=20,
            height=500
        )

        # Panel derecho con título y descripción
        self.title_label = ft.Text(
            value="Seleccione una ficha",
            size=24,
            weight=ft.FontWeight.BOLD,
            color=ft.colors.BLUE
        )

        self.description_text = ft.TextField(
            multiline=True,
            min_lines=10,
            max_lines=20,
            read_only=True,
            border_color=ft.colors.BLUE_200,
            bgcolor=ft.colors.WHITE10,
            width=400
        )

        # Panel derecho completo
        right_panel = ft.Container(
            content=ft.Column(
                controls=[
                    self.title_label,
                    ft.Divider(height=20, color=ft.colors.TRANSPARENT),
                    self.description_text
                ],
                spacing=10,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER
            ),
            padding=20
        )

        # Contenedor principal
        main_container = ft.Container(
            content=ft.Row(
                controls=[
                    # Panel izquierdo con borde
                    ft.Container(
                        content=ft.Column(
                            controls=[
                                ft.Text("Mis Fichas", size=20, weight=ft.FontWeight.BOLD),
                                self.fichas_list
                            ]
                        ),
                        width=300,
                        border=ft.border.all(2, ft.colors.BLUE_200),
                        border_radius=10,
                        padding=10
                    ),
                    ft.VerticalDivider(width=20, color=ft.colors.TRANSPARENT),
                    # Panel derecho con borde
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

        # Cargar fichas al iniciar
        self.load_fichas()

        # Crear y agregar el botón flotante
        self.page.floating_action_button = ft.FloatingActionButton(
            icon=ft.icons.ADD,
            on_click=lambda _: self.page.go("/newCard"),
            tooltip="Agregar nueva tarjeta",  # Tooltip para el botón
            bgcolor=ft.colors.BLUE,  # Color de fondo del botón
        )

        return main_container

def card_view(page: Page):
    return CardView(page)