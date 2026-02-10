import json
import os
import sys
import shutil
from typing import Optional, Dict, Any, List
import glob
from cardfile.config.runtime import get_data_dir


class Config:
    _instance = None

    def __new__(cls, *args, **kwargs):
        if not cls._instance:
            cls._instance = super(Config, cls).__new__(cls)
        return cls._instance

    def __init__(self, config_file="config.json"):
        if hasattr(self, "_initialized") and self._initialized:
            return
        
        self._initialized = True
        self.config_filename = config_file
        # Determine if we are in portable mode by checking for config.json in app dir
        self.is_portable = self._check_if_portable()
        self.base_data_dir = get_data_dir("Cardfile", portable=self.is_portable)
        
        # Ensure base data dir exists
        if not os.path.exists(self.base_data_dir):
            os.makedirs(self.base_data_dir, exist_ok=True)

        self.config_file = os.path.join(self.base_data_dir, self.config_filename)
        
        # Initialize default config if not exists
        if not os.path.exists(self.config_file):
            self.config_data = self._get_defaults()
            self.save_config()
        else:
            self.config_data = self.load_config()

        self.available_languages = self._discover_languages()
        self.language_names = self._get_language_names()
        self.current_language = self.get("app.language.default", "es")
        self.current_theme = self.get("app.theme", "snow")
        self.translations: Dict[str, Any] = {}
        self._load_translations()

    def _get_defaults(self) -> Dict[str, Any]:
        """Return default configuration."""
        return {
            "app": {
                "name": "Cardfile",
                "language": {
                    "default": "es",
                    "path": "./lang"
                },
                "theme": "snow",
                "auth": {
                    "require_login": True,
                    "session_expiry_days": 7
                },
                "debug": False
            },
            "database": {
                "uri": "sqlite:///database.db"
            }
        }

    def _check_if_portable(self) -> bool:
        """Check if config.json exists in the application root directory."""
        if getattr(sys, "frozen", False):
            app_dir = os.path.dirname(sys.executable)
        else:
            # Look in src/cardfile for config.json
            app_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
        
        # Also check Root/config.json for compatibility or if mounted in /app in Docker
        if os.path.exists(os.path.join(app_dir, self.config_filename)):
            return True
        
        root_dir = os.path.abspath(os.path.join(app_dir, "..", ".."))
        return os.path.exists(os.path.join(root_dir, self.config_filename))

    def _base_dir(self) -> str:
        """Returns the project root directory."""
        if getattr(sys, "frozen", False):
            return os.path.dirname(sys.executable)
        # From src/cardfile/config/config.py -> .. (config) -> .. (cardfile) -> .. (src) -> .. (root)
        return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))

    def _resolve_path(self, path: str) -> str:
        """Resolves a relative path to either the data directory or the bundle directory."""
        if os.path.isabs(path):
            return path
        
        # Try relative to the script location (for bundled assets like lang)
        base_dir = self._base_dir()
        candidate = os.path.join(base_dir, path)
        if os.path.exists(candidate):
            return candidate
            
        bundle_dir = getattr(sys, "_MEIPASS", None)
        if bundle_dir:
            bundled = os.path.join(bundle_dir, path)
            if os.path.exists(bundled):
                return bundled
                
        return candidate

    def _discover_languages(self) -> List[str]:
        lang_path = self._resolve_path(self.get("app.language.path", "./lang"))
        if not os.path.exists(lang_path):
            # Try to find it in the source tree if not found
            src_lang_path = os.path.abspath(os.path.join(self._base_dir(), "lang"))
            if os.path.exists(src_lang_path):
                lang_path = src_lang_path
            else:
                os.makedirs(lang_path, exist_ok=True)
            
        pattern = os.path.join(lang_path, "*.json")
        language_files = glob.glob(pattern)
        languages = [os.path.splitext(os.path.basename(f))[0] for f in language_files]
        
        if not languages:
            languages = ["es"]
        return sorted(languages)

    def _get_language_names(self) -> Dict[str, str]:
        """
        Retorna un diccionario con los nombres de los idiomas
        """
        return {
            "es": "Español",
            "en": "English",
            "pt_BR": "Português",
            "fr": "Français",
            "de": "Deutsch",
            "ru": "Русский",
            "zh": "中文",
        }

    def get_language_options(self) -> List[Dict[str, str]]:
        """
        Retorna una lista de opciones de idioma para el dropdown
        Formato: [{"value": "es", "text": "Español"}, ...]
        """
        return [
            {"value": lang_code, "text": self.language_names.get(lang_code, lang_code)}
            for lang_code in self.available_languages
            if lang_code in self.language_names
        ]

    def _load_translations(self) -> None:
        """Carga el archivo de traducciones según el idioma configurado"""
        lang_path = self._resolve_path(self.get("app.language.path", "./lang"))
        lang_file = os.path.join(lang_path, f"{self.current_language}.json")
        
        if not os.path.exists(lang_file):
            # Try fallback to 'es' in the same path
            lang_file = os.path.join(lang_path, "es.json")
            if not os.path.exists(lang_file):
                # Hard fallback: return empty dict if no translations found
                self.translations = {}
                return
        
        with open(lang_file, 'r', encoding='utf-8') as file:
            self.translations = json.load(file)

    def set_language(self, language: str) -> bool:
        """
        Cambia el idioma actual si está disponible
        Retorna True si el cambio fue exitoso, False si el idioma no está disponible
        """
        if language in self.available_languages:
            self.current_language = language
            self._load_translations()
            self.set("app.language.default", language)
            return True
        return False

    def get_text(self, key: str, default: str = "") -> str:
        """
        Obtiene un texto traducido según la clave proporcionada
        Ejemplo: config.get_text("login.title")
        """
        keys = key.split(".")
        data = self.translations
        for k in keys:
            if isinstance(data, dict) and k in data:
                data = data[k]
            else:
                return default
        return str(data) if data is not None else default

    def load_config(self):
        """Carga la configuración desde un archivo JSON"""
        if not os.path.exists(self.config_file):
            return self._get_defaults()
        with open(self.config_file, 'r', encoding='utf-8') as file:
            return json.load(file)

    def get(self, key, default=None):
        """Obtiene un valor del JSON dado su clave, devuelve default si no existe"""
        keys = key.split(".")
        data = self.config_data
        for k in keys:
            if isinstance(data, dict) and k in data:
                data = data[k]
            else:
                return default
        return data

    def set(self, key, value):
        """Establece un valor en el JSON dado su clave y lo guarda"""
        keys = key.split(".")
        data = self.config_data
        for k in keys[:-1]:
            if k not in data or not isinstance(data[k], dict):
                data[k] = {}
            data = data[k]
        data[keys[-1]] = value
        self.save_config()

    def save_config(self):
        """Guarda la configuración actual en el archivo JSON"""
        try:
            with open(self.config_file, 'w', encoding='utf-8') as file:
                json.dump(self.config_data, file, indent=4, ensure_ascii=False)
        except Exception as e:
            print(f"Error saving config file: {e}")

    def get_database_uri(self) -> str:
        """Get the database URI, making sure it points to the correct subdirectory if relative."""
        uri = self.get("database.uri", "sqlite:///database.db")
        if uri.startswith("sqlite:///"):
            db_file = uri[10:]
            if not os.path.isabs(db_file):
                return f"sqlite:///{os.path.join(self.base_data_dir, db_file)}"
        return uri

    def set_theme(self, theme_name: str) -> None:
        """Cambia el tema actual"""
        self.current_theme = theme_name
        self.set("app.theme", theme_name)
