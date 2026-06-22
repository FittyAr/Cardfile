# Compilación y releases

En esta guía explico cómo compilo Cardfile-py y cómo publico releases.

## Compilación local

### Windows

1. Creo el entorno virtual e instalo dependencias:

```bash
python -m venv .venv
.venv\Scripts\activate
pip install --upgrade pip
pip install -r requirements.txt
pip install pyinstaller
```

### Linux / macOS

1. Creo el entorno virtual e instalo dependencias:

```bash
python -m venv .venv
# Para bash / zsh:
source .venv/bin/activate
# Para fish:
source .venv/bin/activate.fish

pip install --upgrade pip
pip install -r requirements.txt
pip install pyinstaller
```

### Mantenimiento de dependencias

Actualizo dependencias con:

```bash
pip install --upgrade -r requirements.txt
```

Regenero requirements.txt con:

```bash
pip freeze > requirements.txt
```

2. Compilo:

En Windows:
```powershell
.\run.ps1 -Build
```

En Linux / macOS:
```bash
./run.sh --build
```

*(Nota: Ejecutar los scripts sin argumentos inicia el menú de la herramienta interactiva de gestión).*

3. Resultado:

- El ejecutable queda en dist/Cardfile.exe
- config.json y lang deben estar disponibles junto al ejecutable

## Publicar una versión

1. Creo el tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

2. El workflow genera el release automáticamente con los artefactos.
