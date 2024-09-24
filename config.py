import json
import os

DATABASE_URI = 'sqlite:///database.db'


class Config:
    def __init__(self, config_file="config.json"):
        self.config_file = config_file
        self.config_data = self.load_config()

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