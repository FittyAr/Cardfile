# Cardfile
A new tab file in the style of the extinct Windows 3.1

# Activando ambiente virtual
Para crear un ambiente virtual, ejecuta uno de los siguientes comandos:
```
python -m venv venv
```
o
```
python -m venv c:\miproyecto\venv
```
    Virtual Enviroment:
        Es el concepto usado de python para referirse a un proyecto.
        Cada proyecto tiene un conjunto de librerias pertinentes como una version de python asociada.

# Luego activar el ambiente virtual
CMD
```
  .\venv\Scripts\Activate.bat (for CMD)
```
PowerShell
```
  .\venv\Scripts\Activate.ps1 (for PowerShell)
```
# Install dependencies
Ejecuta los siguientes comandos para instalar y actualizar las dependencias:
```
    python.exe -m pip install --upgrade pip
    pip install flet        flet.dev/
    pip install peewee      peewee-orm.com/
    pip install SQLAlchemy      sqlalchemy.org/
```

# Update dependencies (Only if necessary)
```
    python.exe -m pip install --upgrade pip
    pip install flet --upgrade
    pip install peewee --upgrade
    pip install SQLAlchemy --upgrade
```

# Archivo `requirements.txt`
Este archivo contiene las dependencias de un proyecto o ambiente virtual. Por convención, se llama requirements.txt.

       - Para instalar esas dependencias: 
```
pip install -r requirements.txt
```
    - Para generar este archivo nuevo:
```
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