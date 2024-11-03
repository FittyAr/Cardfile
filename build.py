import os
import shutil
import subprocess

# Limpiar carpetas anteriores
if os.path.exists('build'):
    shutil.rmtree('build')
if os.path.exists('dist'):
    shutil.rmtree('dist')

# Compilar
subprocess.run(['pyinstaller', 'build.spec', '--clean'])

print("Compilación completada. Revisa la carpeta 'dist'") 