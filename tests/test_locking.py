import unittest
from config.locking import mask_title, hash_lock_password, verify_lock_password, get_user_locking_settings
from config.config import Config


class LockingTests(unittest.TestCase):
    def test_mask_title_short(self):
        self.assertEqual(mask_title("abc", 5), "abc")

    def test_mask_title_long(self):
        self.assertEqual(mask_title("abcdef", 5), "abcde•")

    def test_mask_title_zero(self):
        self.assertEqual(mask_title("abcdef", 0), "••••••")

    def test_password_hash_verify(self):
        password = "clave123"
        hashed = hash_lock_password(password)
        self.assertTrue(verify_lock_password(password, hashed))
        self.assertFalse(verify_lock_password("otra", hashed))

    def test_user_locking_overrides(self):
        class DummyUser:
            locking_enabled = True
            locking_auto_lock_seconds = 12
            locking_mask_visible_chars = 3
            locking_password_hash = "hash"

        settings = get_user_locking_settings(Config(), DummyUser())
        self.assertTrue(settings["enabled"])
        self.assertEqual(settings["auto_lock_seconds"], 12)
        self.assertEqual(settings["mask_visible_chars"], 3)
        self.assertEqual(settings["password_hash"], "hash")


if __name__ == "__main__":
    unittest.main()
