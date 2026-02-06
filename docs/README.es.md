# Cardfile

Cardfile es un gestor de fichas/tarjetas multiplataforma inspirado en el estilo clásico de Windows 3.1, construido con Flet 1.0 Beta. Combina flujo CRUD rápido, edición Markdown, autenticación y soporte multi‑idioma.

## Destacados

- Autenticación con hashing de contraseñas
- Ciclo completo de fichas (crear, editar, eliminar, restaurar)
- Editor Markdown con vista previa en tiempo real
- Papelera de reciclaje para recuperación
- Interfaz multi‑idioma
- Soporte en escritorio y web

## Requisitos

- Python 3.10+
- Docker (opcional)

## Inicio rápido

### Ejecución local

1. Crear entorno virtual:
```bash
python -m venv venv
```

2. Activarlo:
   - Windows (CMD): `.\venv\Scripts\Activate.bat`
   - Windows (PowerShell): `.\venv\Scripts\Activate.ps1`
   - Linux/Mac: `source venv/bin/activate`

3. Instalar dependencias:
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. Ejecutar:
```bash
python main.py
```

La app abre en `http://localhost:8550`.

### Docker

```bash
cd docker
docker-compose up --build
```

## Configuración

Edita `config.json` para base de datos, idioma y modo de ejecución.

## Documentación

- DeepWiki: https://deepwiki.com/FittyAr/Cardfile
- Más documentación en `docs/`.

## Colaborar

¡Las contribuciones son bienvenidas! Puedes:

- Reportar issues con pasos claros
- Enviar PRs con cambios acotados
- Mejorar traducciones

## Licencia

Ver [LICENSE](../LICENSE) para más detalles.
