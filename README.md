# Cardfile-py

Aplicación de gestión de fichas/tarjetas inspirada en el estilo de Windows 3.1, construida con Flet 1.0 Beta.

## Características

- Sistema de autenticación con encriptación de contraseñas
- Gestión completa de fichas (CRUD)
- Editor Markdown con preview en tiempo real
- Papelera de reciclaje para recuperar elementos eliminados
- Soporte multiidioma (Español, Inglés, Portugués BR, Francés, Alemán, Ruso, Chino)
- Interfaz moderna construida con Flet 1.0 Beta

## Requisitos

- Python 3.10 o superior
- Docker (opcional, para ejecución en contenedor)

## Instalación y Ejecución

### Opción 1: Ejecución Local

1. **Crear entorno virtual**:
```bash
python -m venv venv
```

2. **Activar entorno virtual**:
   - Windows (CMD): `.\venv\Scripts\Activate.bat`
   - Windows (PowerShell): `.\venv\Scripts\Activate.ps1`
   - Linux/Mac: `source venv/bin/activate`

3. **Instalar dependencias**:
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. **Ejecutar aplicación**:
```bash
python main.py
```

La aplicación se abrirá automáticamente en tu navegador en `http://localhost:8550`

### Opción 2: Ejecución con Docker (Recomendado)

1. **Construir y ejecutar con Docker Compose**:
```bash
cd docker
docker-compose up --build
```

2. **Acceder a la aplicación**:
Abre tu navegador en `http://localhost:8550`

Para detener el contenedor:
```bash
docker-compose down
```

## Configuración

La configuración se encuentra en `config.json`. Puedes modificar:

- Tipo de base de datos (SQLite por defecto)
- Credenciales de base de datos remota (si aplica)
- Idioma por defecto
- Modo de inicio (Web, Desktop, Mobile)

## Estructura del Proyecto

```
Cardfile-py/
├── config/          # Configuración y rutas
├── data/            # Modelos y repositorios de datos
├── View/            # Componentes de interfaz de usuario
├── lang/            # Archivos de traducción
├── docker/          # Configuración Docker
├── main.py          # Punto de entrada
└── requirements.txt # Dependencias Python
```

## Documentación

- **ANALISIS.md**: Análisis técnico completo del proyecto
- **MIGRACION.md**: Guía de migración a Flet 1.0 Beta

## Tecnologías Utilizadas

- **Flet 0.80.0** (1.0 Beta): Framework para aplicaciones multiplataforma
- **SQLAlchemy 2.0+**: ORM para Python
- **bcrypt**: Encriptación de contraseñas
- **Markdown-it-py**: Renderizado de Markdown

## Desarrollo

### Actualización de Dependencias

```bash
pip install --upgrade -r requirements.txt
```

### Generar nuevo requirements.txt

```bash
pip freeze > requirements.txt
```

## Notas Importantes

- La aplicación ahora usa **Flet 1.0 Beta (0.80.0)** con modo web mediante `Browser()`
- La base de datos SQLite se crea automáticamente en la primera ejecución
- El puerto por defecto es **8550** (configurable en `config.json`)

## Enlaces Útiles

- [Flet Documentation](https://docs.flet.dev/)
- [Flet 1.0 Beta Blog](https://flet.dev/blog/flet-1-0-beta)
- [SQLAlchemy](https://www.sqlalchemy.org/)
- [Flet GitHub](https://github.com/flet-dev/flet)

## Licencia

Ver archivo LICENSE para más detalles.