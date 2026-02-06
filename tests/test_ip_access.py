import unittest
from config.security import normalize_allowed_ips, is_ip_allowed


class IpAccessTests(unittest.TestCase):
    def test_allows_all_with_zero_ip(self):
        self.assertTrue(is_ip_allowed(["0.0.0.0"], "10.0.0.1"))

    def test_allows_specific_ip(self):
        self.assertTrue(is_ip_allowed(["192.168.1.10"], "192.168.1.10"))

    def test_denies_ip_not_in_list(self):
        self.assertFalse(is_ip_allowed(["192.168.1.10"], "192.168.1.11"))

    def test_denies_when_client_ip_missing(self):
        self.assertFalse(is_ip_allowed(["192.168.1.10"], None))

    def test_string_list_is_parsed(self):
        self.assertTrue(is_ip_allowed("192.168.1.10, 10.0.0.2", "10.0.0.2"))

    def test_invalid_type_defaults_to_allow_all(self):
        self.assertTrue(is_ip_allowed({"a": "b"}, "10.0.0.2"))

    def test_normalize_empty_list_defaults_to_allow_all(self):
        self.assertEqual(normalize_allowed_ips([]), ["0.0.0.0"])


if __name__ == "__main__":
    unittest.main()
