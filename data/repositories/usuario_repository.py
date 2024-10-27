from data.models.usuario import Usuario
from data.database.connection import get_session

class UsuarioRepository:
    def __init__(self):
        self.session = get_session()

    def add_usuario(self, usuario):
        self.session.add(usuario)
        self.session.commit()

    def get_all_usuarios(self):
        return self.session.query(Usuario).all()
    
    def find_by_email(self, email):
        return self.session.query(Usuario).filter_by(Usuario.email==email)
    
    def find_by_email_and_password(self, email, contraseña):
        usuario = self.session.query(Usuario).filter_by(Usuario.email==email, Usuario.contraseña==contraseña)
        return usuario

    def close(self):
        self.session.close()