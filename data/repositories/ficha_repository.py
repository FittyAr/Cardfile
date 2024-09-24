from data.models.ficha import Ficha
from data.database.connection import get_session

class FichaRepository:
    def __init__(self):
        self.session = get_session()

    def add_ficha(self, ficha):
        self.session.add(ficha)
        self.session.commit()

    def get_all_fichas(self):
        return self.session.query(Ficha).all()

    def close(self):
        self.session.close()