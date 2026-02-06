# Compilación y releases

## Compilación local (Windows)

1. Crear entorno virtual e instalar dependencias:

```bash
python -m venv .venv
.venv\Scripts\activate
pip install --upgrade pip
pip install -r requirements.txt
pip install pyinstaller
```

### Mantenimiento de dependencias

Actualizar dependencias:

```bash
pip install --upgrade -r requirements.txt
```

Regenerar requirements.txt:

```bash
pip freeze > requirements.txt
```

2. Compilar:

```bash
python build.py
```

3. Resultado:

- El ejecutable queda en dist/Cardfile.exe
- config.json y lang deben estar disponibles junto al ejecutable

## GitHub Actions (build y release automático)

Crear el workflow en .github/workflows/release.yml con este contenido:

```yaml
name: build-and-release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: "3.12"

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install -r requirements.txt
          pip install pyinstaller

      - name: Build
        run: |
          python build.py

      - name: Package
        run: |
          mkdir release
          copy dist\\Cardfile.exe release\\Cardfile.exe
          copy config.json release\\config.json
          xcopy /E /I lang release\\lang

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            release/Cardfile.exe
            release/config.json
            release/lang/**
```

## Publicar una versión

1. Crear tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

2. El workflow genera el release automáticamente con los artefactos.
