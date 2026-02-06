from sqlalchemy import Column, Integer, String, Boolean, DateTime, func
from sqlalchemy.sql import text
from sqlalchemy.orm import relationship
from data.models.base import Base

class Usuario(Base):
    __tablename__ = 'usuarios'

    id = Column(Integer, primary_key=True)
    nombre = Column(String(100), nullable=False)
    email = Column(String(255), unique=True, nullable=False)
    contraseña = Column(String(255), nullable=False)  # Almacenar hash de la contraseña
    is_active = Column(Boolean, default=True, nullable=False)
    last_login = Column(DateTime)
    locking_enabled = Column(Boolean, nullable=True)
    locking_auto_lock_seconds = Column(Integer, nullable=True)
    locking_mask_visible_chars = Column(Integer, nullable=True)
    locking_password_hash = Column(String(255), nullable=True)
    created_at = Column(DateTime, server_default=text('CURRENT_TIMESTAMP'), nullable=False)
    updated_at = Column(DateTime, server_default=text('CURRENT_TIMESTAMP'), onupdate=func.now(), nullable=False)
    
    fichas = relationship("Ficha", back_populates="usuario")
