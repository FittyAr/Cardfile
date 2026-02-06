import bcrypt


def get_locking_settings(config):
    enabled = bool(config.get("app.locking.enabled", False))
    auto_lock_seconds = config.get("app.locking.auto_lock_seconds", 30)
    mask_visible_chars = config.get("app.locking.mask_visible_chars", 5)
    password_hash = config.get("app.locking.password_hash", "")
    return {
        "enabled": enabled,
        "auto_lock_seconds": max(int(auto_lock_seconds), 0),
        "mask_visible_chars": max(int(mask_visible_chars), 0),
        "password_hash": password_hash or "",
    }


def get_user_locking_settings(config, user=None):
    settings = get_locking_settings(config)
    if not user:
        return settings
    if getattr(user, "locking_enabled", None) is not None:
        settings["enabled"] = bool(user.locking_enabled)
    if getattr(user, "locking_auto_lock_seconds", None) is not None:
        settings["auto_lock_seconds"] = max(int(user.locking_auto_lock_seconds), 0)
    if getattr(user, "locking_mask_visible_chars", None) is not None:
        settings["mask_visible_chars"] = max(int(user.locking_mask_visible_chars), 0)
    if getattr(user, "locking_password_hash", None) is not None:
        settings["password_hash"] = user.locking_password_hash or ""
    return settings


def hash_lock_password(password: str) -> str:
    return bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")


def verify_lock_password(password: str, password_hash: str) -> bool:
    if not password_hash:
        return False
    try:
        return bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8"))
    except Exception:
        return False


def mask_title(title: str, visible_chars: int) -> str:
    if not title:
        return ""
    visible = max(visible_chars, 0)
    if len(title) <= visible:
        return title
    return f"{title[:visible]}{'•' * (len(title) - visible)}"
