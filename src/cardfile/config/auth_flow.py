ROUTE_MAPPING = {
    "/": "/",
    "/card": "/Card",
    "/login": "/Login",
    "/newuser": "/newUser",
    "/newcard": "/newCard",
    "/editcard": "/editCard",
    "/recycle": "/recycle",
    "/setup": "/Setup",
}

VALID_ROUTES = {"/Card", "/Login", "/newUser", "/Setup"}
MODAL_ROUTES = {"/newCard", "/editCard", "/recycle"}


def normalize_route(route: str | None) -> str:
    if not route:
        return "/"
    route = route.strip()
    if not route:
        return "/"
    lower = route.lower()
    return ROUTE_MAPPING.get(lower, route)


def resolve_route(
    requested_route: str | None,
    is_authenticated: bool,
    require_login: bool,
    is_first_run: bool,
    needs_account_creation: bool = False,
) -> str:
    normalized = normalize_route(requested_route)
    if is_first_run:
        return "/Setup"
    if needs_account_creation:
        if normalized == "/newUser":
            return "/newUser"
        return "/newUser"
    if not require_login:
        if normalized in {"/", "/Login", "/newUser"}:
            return "/Card"
        if normalized in VALID_ROUTES:
            return normalized
        if normalized in MODAL_ROUTES:
            return "/Card"
        return "/Card"
    if normalized == "/newUser":
        return "/Card" if is_authenticated else "/newUser"
    if normalized in {"/", "/Login"}:
        return "/Card" if is_authenticated else "/Login"
    if normalized in MODAL_ROUTES:
        return "/Card" if is_authenticated else "/Login"
    if normalized in VALID_ROUTES:
        return normalized if is_authenticated else ("/newUser" if normalized == "/newUser" else "/Login")
    return "/Card" if is_authenticated else "/Login"
