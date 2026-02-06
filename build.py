import os
import shutil
import subprocess

def safe_rmtree(path: str) -> None:
    if not os.path.exists(path):
        return
    try:
        shutil.rmtree(path)
    except PermissionError:
        print(f"No se pudo eliminar '{path}'. Cierra la app y vuelve a intentar.")
    except Exception as e:
        print(f"No se pudo eliminar '{path}': {e}")

safe_rmtree("build")
safe_rmtree("dist")

subprocess.run(["pyinstaller", "build.spec", "--clean"])

print("Compilación completada. Revisa la carpeta 'dist'")
