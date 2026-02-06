import unittest
from config.auth_flow import resolve_route, normalize_route


class AuthFlowTests(unittest.TestCase):
    def test_first_run_forces_new_user(self):
        self.assertEqual(resolve_route("/Login", False, True, True), "/newUser")

    def test_login_redirects_when_authenticated(self):
        self.assertEqual(resolve_route("/Login", True, True, False), "/Card")

    def test_card_redirects_when_not_authenticated(self):
        self.assertEqual(resolve_route("/Card", False, True, False), "/Login")

    def test_root_redirects_when_not_authenticated(self):
        self.assertEqual(resolve_route("/", False, True, False), "/Login")

    def test_new_user_allowed_when_not_authenticated(self):
        self.assertEqual(resolve_route("/newUser", False, True, False), "/newUser")

    def test_new_user_redirects_when_authenticated(self):
        self.assertEqual(resolve_route("/newUser", True, True, False), "/Card")

    def test_no_login_mode_skips_login(self):
        self.assertEqual(resolve_route("/Login", False, False, False), "/Card")

    def test_modal_routes_require_auth(self):
        self.assertEqual(resolve_route("/newCard", False, True, False), "/Login")
        self.assertEqual(resolve_route("/newCard", True, True, False), "/Card")

    def test_unknown_route_fallback(self):
        self.assertEqual(resolve_route("/unknown", True, True, False), "/Card")
        self.assertEqual(resolve_route("/unknown", False, True, False), "/Login")

    def test_normalize_route(self):
        self.assertEqual(normalize_route(" /LoGiN "), "/Login")


if __name__ == "__main__":
    unittest.main()
