from cardfile.data.models.base import Base
from cardfile.data.database.connection import engine
from sqlalchemy import inspect, text
from cardfile.data.models.usuario import Usuario
from cardfile.data.models.ficha import Ficha
from cardfile.data.models.config import AppConfig

def init_db():
    Base.metadata.create_all(engine)
    ensure_ficha_lock_columns()
    ensure_usuario_lock_columns()

def ensure_ficha_lock_columns():
    inspector = inspect(engine)
    if "fichas" not in inspector.get_table_names():
        return
    columns = [col["name"] for col in inspector.get_columns("fichas")]
    if "is_locked" not in columns:
        with engine.begin() as conn:
            conn.execute(text("ALTER TABLE fichas ADD COLUMN is_locked BOOLEAN NOT NULL DEFAULT 0"))

def ensure_usuario_lock_columns():
    inspector = inspect(engine)
    if "usuarios" not in inspector.get_table_names():
        return
    columns = [col["name"] for col in inspector.get_columns("usuarios")]
    with engine.begin() as conn:
        if "locking_enabled" not in columns:
            conn.execute(text("ALTER TABLE usuarios ADD COLUMN locking_enabled BOOLEAN"))
        if "locking_auto_lock_seconds" not in columns:
            conn.execute(text("ALTER TABLE usuarios ADD COLUMN locking_auto_lock_seconds INTEGER"))
        if "locking_mask_visible_chars" not in columns:
            conn.execute(text("ALTER TABLE usuarios ADD COLUMN locking_mask_visible_chars INTEGER"))
        if "locking_password_hash" not in columns:
            conn.execute(text("ALTER TABLE usuarios ADD COLUMN locking_password_hash VARCHAR(255)"))
