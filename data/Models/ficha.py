from sqlalchemy import Column, Integer, String, Boolean, ForeignKey, DateTime, func
from sqlalchemy.sql import text
from sqlalchemy.orm import relationship
from data.models.base import Base

class Ficha(Base):
    __tablename__ = 'fichas'

    id = Column(Integer, primary_key=True)
    title = Column(String(100), nullable=False)
    descripcion = Column(String, nullable=True)
    usuario_id = Column(Integer, ForeignKey('usuarios.id'), nullable=False)
    is_active = Column(Boolean, nullable=False, default=True)
    created_at = Column(DateTime, server_default=text('CURRENT_TIMESTAMP'), nullable=False)
    updated_at = Column(DateTime, server_default=text('CURRENT_TIMESTAMP'), onupdate=func.now(), nullable=False)

    usuario = relationship("Usuario", back_populates="fichas")