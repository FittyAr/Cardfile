# Cardfile
A new tab file in the style of the extinct Windows 3.1

# Activando ambiente virtual
Para crear un ambiente virtual, ejecuta uno de los siguientes comandos:

```bash
python -m venv venv
```

o

```bash
python -m venv c:\miproyecto\venv
```

Virtual Enviroment:
- Es el concepto en Python para referirse a un proyecto aislado. Cada proyecto tiene un conjunto de librerías y versiones de Python asociadas.

# Luego activar el ambiente virtual
- CMD

```bash
.\venv\Scripts\Activate.bat (for CMD)
```

- PowerShell

```powershell
.\venv\Scripts\Activate.ps1 (for PowerShell)
```

# Install dependencies
Ejecuta los siguientes comandos para instalar y actualizar las dependencias:
```bash
python.exe -m pip install --upgrade pip
pip install flet        flet.dev/
pip install peewee      peewee-orm.com/
pip install SQLAlchemy      sqlalchemy.org/
```

# Update dependencies (Only if necessary)
```bash
python.exe -m pip install --upgrade pip
pip install flet --upgrade
pip install peewee --upgrade
pip install SQLAlchemy --upgrade
```

# Archivo `requirements.txt`
Este archivo contiene las dependencias de un proyecto o ambiente virtual. Por convención, se llama requirements.txt.

- Para instalar esas dependencias: 

```bash
pip install -r requirements.txt
```

- Para generar este archivo nuevo:

```bash
pip freeze > requirements.txt
```
# Otras librerias de interes:

Estudiar:
    Duck Typing
    Enlaces Dinamicos y estaticos
    Tipo real y dedicado


# Atajos de teclado utiles en VS Code

    ctrl + j  =   Mostrar ocultar terminal
    ctrk + k , ctrk + c = comentar codigo linea por linea
    ctrk + k , ctrk + u = descomentar codigo linea por linea

flet -r <name of file>.py

flet run