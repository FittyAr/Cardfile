from data.models.base import Base
from data.database.connection import engine
from data.models.usuario import Usuario
from data.models.ficha import Ficha

def init_db():
    Base.metadata.create_all(engine)