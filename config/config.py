import json
import os
from typing import Optional, Dict, Any

DATABASE_URI = 'sqlite:///database.db'


class Config:
    def __init__(self, config_file="config.json"):
        self.config_file = config_file
        self.config_data = self.load_config()
        self.current_language = self.get("app.language.default", "es")
        self.translations: Dict[str, Any] = {}
        self._load_translations()

    def _load_translations(self) -> None:
        """Carga el archivo de traducciones según el idioma configurado"""
        lang_path = self.get("app.language.path", "./lang")
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
        available_languages = self.get("app.language.available", ["es"])
        if language in available_languages:
            self.current_language = language
            self._load_translations()
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
            if k in data:
                data = data[k]
            else:
                return default
        return data