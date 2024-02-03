# Cardfile
A new tab file in the style of the extinct Windows 3.1

# Activando ambiente virtual
```
python -m venv venv
o
python -m venv c:\miproyecto\venv
```
    Virtual Enviroment:
        Es el concepto usado de python para referirse a un proyecto.
        Cada proyecto tiene un conjunto de librerias pertinentes como una version de python asociada.

 # Luego activar el ambiente virtual
 ```
  .\venv\Scripts\Activate.bat (for CMD)
  .\venv\Scripts\Activate.ps1 (for PowerShell)
 ```
# Install dependencies
    pip install flet        flet.dev/
    pip install peewee      peewee-orm.com/

# Update dependencies (Only if necessary)
    python.exe -m pip install --upgrade pip
    pip install flet --upgrade
    pip install peewee --upgrade

 # Archivo que contiene las dependencias de un proyecto (o ambiente virtual)
    Este archivo por convencion se suele llamar requirements.txt
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