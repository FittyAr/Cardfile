from data.database.setup import init_db
from data.repositories.usuario_repository import UsuarioRepository
from data.models.usuario import Usuario
from data.database.connection import engine
from config.config import Config
import bcrypt

def initialize_db():
    # Inicializar la base de datos
    init_db()

    # Crear un repositorio de usuarios
    usuario_repo = UsuarioRepository()

    try:
        config = Config()

        # Verificar si ya existe un usuario
        if usuario_repo.get_all_usuarios():
            print("La base de datos ya tiene usuarios registrados.")
            return

        # Obtener datos del usuario inicial desde la configuración
        username = config.get("database.Init_Data.Username")
        email = config.get("database.Init_Data.Email")
        password = config.get("database.Init_Data.Password")

        # Generar hash de la contraseña
        hashed_password = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt()).decode('utf-8')

        # Agregar un nuevo usuario
        nuevo_usuario = Usuario(nombre=username, email=email, contraseña=hashed_password)
        #usuario_repo.add_usuario(nuevo_usuario)
        print("Nuevo usuario agregado exitosamente.")

        # Obtener todos los usuarios
        usuarios = usuario_repo.get_all_usuarios()
        for usuario in usuarios:
            print(f"ID: {usuario.id}, Nombre: {usuario.nombre}, Email: {usuario.email}, Activo: {usuario.is_active}")
    
    except Exception as e:
        usuario_repo.session.rollback()
        print(f"Error al agregar el usuario o recuperar la lista de usuarios: {e}")

    finally:
        # Cerrar la sesión del repositorio
        usuario_repo.close()

if __name__ == "__main__":
    initialize_db()
