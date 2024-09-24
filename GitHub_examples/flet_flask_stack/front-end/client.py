""" Let's do a little client side UI"""

""" these are the modules needed"""
import flet
from flet import (
    Page,
    Text,
    View,
    Column,
    Container,
    LinearGradient,
    alignment,
    border_radius,
    padding,
    Row,
    Card,
    TextField,
    FilledButton,
    SnackBar,
)

# Usamos la biblioteca requests para enviar solicitudes GET/POST a nuestro servidor
import requests

def main(page: Page):
    page.title = "Routes Example"
    snack = SnackBar(
        Text("Registration successfull!"),
    )

    def GradientGenerator(start, end):
        ColorGradient = LinearGradient(
            begin=alignment.bottom_left,
            end=alignment.top_right,
            colors=[
                start,
                end,
            ],
        )

        return ColorGradient

    def req_registter(e, email, password):
        # establecemos el correo electrónico y la contraseña como una estructura dict/json 
        # para pasarlo a la solicitud 
        data = {
            "email": email,
            "password": password,
        }

        # aquí, llamamos a una solicitud POST y dirigimos nuestros datos
        # a la URL del servidor y al mismo tiempo pasamos nuestros datos 
        res = requests.post("http://127.0.0.1:5000/register", json=data)

        # ahora, las solicitudes deben ser manejadas por el servidor...

        # si el estado está bien, lo decimos usando un snackbar, de lo contrario notificamos al usuario que algo salió mal. 
        if res.status_code == 201:
            snack.open = True
            page.update()

            # we can check if the user info was stored in the db correctly
            # So the user correclty was inserted in the db using the Python API
        else:
            snack.content.value = "You were not registered! Try again."
            snack.open = True
            page.update()

    def req_login(e, email, password):
        data = {
            "email": email,
            "password": password,
        }

        # igual que con la solicitud anterior, pasamos la información de la misma manera 
        # pero a una ruta diferente, es decir, la ruta /login

        res = requests.post("http://127.0.0.1:5000/login", json=data)

        # Veamos qué hace esta ruta de la API ...

        # si el servidor devuelve un estado 201, podemos iniciar sesión ahora y podemos continuar con la aplicación.
        # A continuación, simplemente muestro la información del usuario para mostrar.
        if res.status_code == 201:

            page.views.append(
                View(
                    f"/{email}",
                    horizontal_alignment="center",
                    vertical_alignment="center",
                    controls=[
                        Column(
                            alignment="center",
                            horizontal_alignment="center",
                            controls=[
                                Text(
                                    "Succssesfuly Logged in!",
                                    size=44,
                                    weight="w700",
                                    text_align="center",
                                ),
                                Text(
                                    # Podemos mostrar la información del usuario utilizando f string
                                    f"Login Information:\nEmail: {email}\nPassword: {password}",
                                    size=32,
                                    weight="w500",
                                    text_align="center",
                                ),
                            ],
                        ),
                    ],
                )
            )

        else:
            snack.content.value = "Could not log in! Try again."
            snack.open = True
            page.update()

        page.update()

    def route_change(route):
        email = TextField(
            label="Email",
            border="underline",
            width=320,
            text_size=14,
        )

        password = TextField(
            label="Password",
            border="underline",
            width=320,
            text_size=14,
            password=True,
            can_reveal_password=True,
        )

        page.views.clear()
        # trabajaremos con vistas, cada 'página' será una vista separada, es decir, página de inicio de sesión, página de registro, etc..

        # comienza con la página de registro
        page.views.append(
            View(
                "/register",
                horizontal_alignment="end",
                vertical_alignment="center",
                padding=padding.only(right=160),
                controls=[
                    Column(
                        alignment="center",
                        controls=[
                            Card(
                                elevation=15,
                                content=Container(
                                    width=550,
                                    height=550,
                                    padding=padding.all(30),
                                    gradient=GradientGenerator("#1f2937", "#111827"),
                                    border_radius=border_radius.all(12),
                                    content=Column(
                                        horizontal_alignment="center",
                                        alignment="start",
                                        controls=[
                                            Text(
                                                "Mock REGISTRATION Form With Python API Back-End",
                                                size=32,
                                                weight="w700",
                                                text_align="center",
                                            ),
                                            Text(
                                                "This Flet web-app/registration page is routed to a python (Flask-based) API. Registering sends a request to the API-specific rout and performs several functions.",
                                                size=14,
                                                weight="w700",
                                                text_align="center",
                                                color="#64748b",
                                            ),
                                            Container(padding=padding.only(bottom=20)),
                                            email,
                                            Container(padding=padding.only(bottom=10)),
                                            password,
                                            Container(padding=padding.only(bottom=20)),
                                            Row(
                                                alignment="center",
                                                spacing=20,
                                                controls=[
                                                    # la interfaz de usuario anterior puede ser replicada, pero lo más importante es aquí:
                                                    # pasamos los valores de correo electrónico y contraseña a la función req_register
                                                    FilledButton(
                                                        content=Text(
                                                            "Register",
                                                            weight="w700",
                                                        ),
                                                        width=160,
                                                        height=40,
                                                        # pasa los valores de entrada a la función que envía las solicitudes HTTP
                                                        on_click=lambda e: req_registter(
                                                            e,
                                                            email.value,
                                                            password.value,
                                                        ),
                                                    ),
                                                    FilledButton(
                                                        content=Text(
                                                            "Login",
                                                            weight="w700",
                                                        ),
                                                        width=160,
                                                        height=40,
                                                        on_click=lambda __: page.go(
                                                            "/login"
                                                        ),
                                                    ),
                                                    snack,
                                                ],
                                            ),
                                        ],
                                    ),
                                ),
                            )
                        ],
                    )
                ],
            )
        )

        if page.route == "/login":
            page.views.append(
                View(
                    "/login",
                    horizontal_alignment="center",
                    vertical_alignment="center",
                    controls=[
                        Column(
                            alignment="center",
                            controls=[
                                Card(
                                    elevation=15,
                                    content=Container(
                                        width=550,
                                        height=550,
                                        padding=padding.all(30),
                                        gradient=GradientGenerator(
                                            "#2f2937", "#251867"
                                        ),
                                        border_radius=border_radius.all(12),
                                        content=Column(
                                            horizontal_alignment="center",
                                            alignment="start",
                                            controls=[
                                                Text(
                                                    "Mock LOGIN Form With Python API Back-End",
                                                    size=32,
                                                    weight="w700",
                                                    text_align="center",
                                                ),
                                                Text(
                                                    "This Flet web-app/registration page is routed to a python (Flask-based) API. Registering sends a request to the API-specific rout and performs several functions.",
                                                    size=14,
                                                    weight="w700",
                                                    text_align="center",
                                                    color="#64748b",
                                                ),
                                                Container(
                                                    padding=padding.only(bottom=20)
                                                ),
                                                email,
                                                Container(
                                                    padding=padding.only(bottom=10)
                                                ),
                                                password,
                                                Container(
                                                    padding=padding.only(bottom=20)
                                                ),
                                                Row(
                                                    alignment="center",
                                                    spacing=20,
                                                    controls=[
                                                        FilledButton(
                                                            content=Text(
                                                                "Login",
                                                                weight="w700",
                                                            ),
                                                            width=160,
                                                            height=40,
                                                            # Ahora, si queremos iniciar sesión, también necesitamos enviar información de vuelta al servidor 
                                                            # y verificar si las credenciales son correctas o si incluso existen.
                                                            on_click=lambda e: req_login(
                                                                e,
                                                                email.value,
                                                                password.value,
                                                            ),
                                                        ),
                                                        FilledButton(
                                                            content=Text(
                                                                "Create acount",
                                                                weight="w700",
                                                            ),
                                                            width=160,
                                                            height=40,
                                                            on_click=lambda __: page.go(
                                                                "/register"
                                                            ),
                                                        ),
                                                        snack,
                                                    ],
                                                ),
                                            ],
                                        ),
                                    ),
                                )
                            ],
                        )
                    ],
                )
            )
        page.update()

    def view_pop(view):
        page.views.pop()
        top_view = page.views[-1]
        page.go(top_view.route)

    page.on_route_change = route_change
    page.on_view_pop = view_pop
    page.go(page.route)

# we can now test this using the web browser
flet.app(target=main, host="localhost", port=9999, view=flet.WEB_BROWSER)