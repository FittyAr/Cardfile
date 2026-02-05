import json
import os
from typing import Optional, Dict, Any, List
import glob

DATABASE_URI = 'sqlite:///database.db'


class Config:
    def __init__(self, config_file="config.json"):
        self.config_file = config_file
        self.config_data = self.load_config()
        self.available_languages = self._discover_languages()
        self.language_names = self._get_language_names()
        self.current_language = self.get("app.language.default", "es")
        self.current_theme = self.get("app.theme", "snow")
        self.translations: Dict[str, Any] = {}
        self._load_translations()

    def _discover_languages(self) -> List[str]:
        """
        Descubre automáticamente los idiomas disponibles en la carpeta lang
        Retorna una lista de códigos de idioma (ej: ['es', 'en', 'pt_BR', 'fr'])
        """
        lang_path = self.get("app.language.path", "./lang")
        if not os.path.exists(lang_path):
            os.makedirs(lang_path)
            
        # Buscar todos los archivos .json en la carpeta lang
        pattern = os.path.join(lang_path, "*.json")
        language_files = glob.glob(pattern)
        
        # Extraer los códigos de idioma de los nombres de archivo
        languages = [os.path.splitext(os.path.basename(f))[0] for f in language_files]
        
        if not languages:
            # Si no hay idiomas, usar español como fallback
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
        lang_path = self.get("app.language.path", "./lang")
        lang_file = os.path.join(lang_path, f"{self.current_language}.json")
        
        if not os.path.exists(lang_file):
            # Si el archivo no existe, intentar usar el idioma por defecto
            self.current_language = self.get("app.language.default", "es")
            lang_file = os.path.join(lang_path, f"{self.current_language}.json")
            
            if not os.path.exists(lang_file):
                raise FileNotFoundError(f"Archivo de idioma '{lang_file}' no encontrado.")
        
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
            raise FileNotFoundError(f"Archivo de configuración '{self.config_file}' no encontrado.")
        
        with open(self.config_file, 'r') as file:
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

    def set_theme(self, theme_name: str) -> None:
        """Cambia el tema actual"""
        self.current_theme = theme_name
        self.set("app.theme", theme_name)
