# Cardfile
A new tab file in the style of the extinct Windows 3.1

## Configuración del Ambiente Virtual

### Creación del Ambiente Virtual

Ejecuta uno de los siguientes comandos:

```bash
python -m venv venv
```

o

```bash
python -m venv c:\miproyecto\venv
```

> **Nota**: Un ambiente virtual en Python es un proyecto aislado con su propio conjunto de librerías y versión de Python.

### Activación del Ambiente Virtual

**Nota**: Es el concepto en Python para referirse a un proyecto aislado. Cada proyecto tiene un conjunto de librerías y versiones de Python asociadas.

# Luego activar el ambiente virtual
- Para CMD

```bash
.\venv\Scripts\Activate.bat
```

- Para PowerShell

```powershell
.\venv\Scripts\Activate.ps1
```

## Gestión de Dependencias

### Instalación de Dependencias

```bash
python.exe -m pip install --upgrade pip
pip install flet peewee SQLAlchemy
```

### Actualización de Dependencias (si es necesario)

```bash
python.exe -m pip install --upgrade pip
pip install --upgrade flet peewee SQLAlchemy
```

### Uso de `requirements.txt`

**Nota**: Este archivo contiene las dependencias de un proyecto o ambiente virtual. Por convención, se llama requirements.txt.

- Para instalar dependencias desde `requirements.txt`:

```bash
pip install -r requirements.txt
```

- Para generar un nuevo `requirements.txt`:

```bash
pip freeze > requirements.txt
```

## Librerías de Interés


## Estudiar

- Duck Typing
- Enlaces Dinámicos y Estáticos
- Tipo Real y Dedicado


## Atajos de Teclado Útiles en VS Code

- `Ctrl + J`: Mostrar/ocultar terminal
- `Ctrl + K, Ctrl + C`: Comentar código línea por línea
- `Ctrl + K, Ctrl + U`: Descomentar código línea por línea

## Ejecución del Proyecto

Para ejecutar el proyecto con Flet:

```bash
flet -r <nombre_del_archivo>.py
```

o simplemente:

```bash
flet run
```

## Enlaces Útiles

- [Flet](https://flet.dev/)
- [Peewee ORM](http://peewee-orm.com/)
- [SQLAlchemy](https://www.sqlalchemy.org/)