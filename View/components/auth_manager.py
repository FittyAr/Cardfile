import flet as ft
from config.config import Config
from data.repositories.usuario_repository import UsuarioRepository
from data.database.connection import get_session
from data.models.usuario import Usuario
import bcrypt
from datetime import datetime

class AuthManager:
    def __init__(self, page: ft.Page):
        self.page = page
        self.config = Config()
        self.repo = UsuarioRepository()

    @property
    def require_login(self) -> bool:
        """Retorna True si la aplicación requiere login según la configuración."""
        return self.config.get("app.auth.require_login", True)

    async def login(self, email, password) -> bool:
        """
        Intenta autenticar al usuario.
        Si es exitoso, guarda la sesión en storage.
        """
        session = get_session()
        try:
            user = session.query(Usuario).filter(Usuario.email == email).first()
            if user and self.verify_password_hash(user.contraseña, password):
                # Guardar sesión de forma persistente
                await self.page.shared_preferences.set("user_id", str(user.id))
                await self.page.shared_preferences.set("username", user.nombre)
                
                # Actualizar último login
                user.last_login = datetime.now()
                session.commit()
                return True
            return False
        finally:
            session.close()

    def verify_password_hash(self, stored_hash: str, provided_password: str) -> bool:
        """Verifica una contraseña contra un hash."""
        try:
            return bcrypt.checkpw(
                provided_password.encode('utf-8'),
                stored_hash.encode('utf-8')
            )
        except Exception:
            return False

    async def update_auth_config(self, require_login: bool, current_password: str) -> bool:
        """
        Cambia la configuración de autenticación.
        Requiere la contraseña del usuario actual para confirmar cambios si se está en modo restringido.
        """
        if self.require_login:
            # Si se requiere login, validar que quien intenta cambiarlo es un usuario válido
            user_id = await self.page.shared_preferences.get("user_id")
            if not user_id:
                return False
                
            session = get_session()
            try:
                user = session.query(Usuario).filter(Usuario.id == int(user_id)).first()
                if not user or not self.verify_password_hash(user.contraseña, current_password):
                    return False
            finally:
                session.close()

        # Guardar nueva configuración
        self.config.set("app.auth.require_login", require_login)
        return True

    async def logout(self):
        """Cierra la sesión y limpia el storage."""
        await self.page.shared_preferences.remove("user_id")
        await self.page.shared_preferences.remove("username")
        await self.page.push_route("/Login")

    async def get_authenticated_user_id(self) -> int:
        """
        Retorna el ID del usuario autenticado.
        Si está en modo 'Sin Login', retorna el ID del usuario 'Guest'.
        """
        if not self.require_login:
            return await self._get_or_create_guest_user()
            
        user_id = await self.page.shared_preferences.get("user_id")
        return int(user_id) if user_id else None

    async def _get_or_create_guest_user(self) -> int:
        """Obtiene o crea el usuario 'Guest' para el modo sin login."""
        session = get_session()
        try:
            guest_email = "guest@cardfile.local"
            user = session.query(Usuario).filter(Usuario.email == guest_email).first()
            if not user:
                user = Usuario(
                    nombre="Guest",
                    email=guest_email,
                    contraseña="no-password", # No se usará para login real
                    is_active=True
                )
                session.add(user)
                session.commit()
                session.refresh(user)
            return user.id
        finally:
            session.close()

    async def is_authenticated(self) -> bool:
        """Verifica si hay una sesión activa en el storage o si el login es opcional."""
        if not self.require_login:
            return True
            
        user_id = await self.page.shared_preferences.get("user_id")
        return user_id is not None

    async def get_current_user(self):
        """Retorna el nombre del usuario actual."""
        if not self.require_login:
            return "Invitado"
        return await self.page.shared_preferences.get("username") or "Anónimo"