#!/usr/bin/env bash
# =============================================================================
#  Cardfile — Development & Test Helper Shell (Linux / macOS)
# =============================================================================

set -euo pipefail

# Silenciar advertencias de accesibilidad de ATK en entornos Linux
export NO_AT_BRIDGE=1

ESC=$(printf '\033')
BOLD="${ESC}[1m"
GREEN="${ESC}[0;32m"
YELLOW="${ESC}[1;33m"
RED="${ESC}[0;31m"
CYAN="${ESC}[0;36m"
RESET="${ESC}[0m"

# ---------------------------------------------------------------------------
# 1. Locate and activate virtual environment (.venv)
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check if in CI (GitHub Actions, etc.)
IS_CI=false
if [[ "${GITHUB_ACTIONS:-}" == "true" || "${CI:-}" == "true" ]]; then
    IS_CI=true
fi

if [ "$IS_CI" = false ]; then
    if [[ ! -d ".venv" ]]; then
        echo -e "${YELLOW}[WARN]${RESET} Entorno virtual .venv no encontrado."
        echo -e "${GREEN}[INFO]${RESET} Creando entorno virtual .venv..."
        if command -v python3 &>/dev/null; then
            python3 -m venv .venv
        elif command -v python &>/dev/null; then
            python -m venv .venv
        else
            echo -e "${RED}[ERROR]${RESET} Python no está instalado en el sistema."
            exit 1
        fi
    fi

    # Activar el entorno virtual
    # Para bash/zsh/sh estándar
    if [[ -f ".venv/bin/activate" ]]; then
        # shellcheck disable=SC1091
        source .venv/bin/activate
    else
        echo -e "${RED}[ERROR]${RESET} No se pudo activar el entorno virtual .venv."
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------
safe_rmtree() {
    local target="$1"
    if [[ -d "$target" ]]; then
        echo -e "${GREEN}[INFO]${RESET} Eliminando $target..."
        rm -rf "$target"
    fi
}

clean_build() {
    safe_rmtree "build"
    safe_rmtree "dist"
}

build_executable() {
    clean_build
    echo -e "\n${GREEN}[INFO]${RESET} Compilando ejecutable con PyInstaller..."
    if pip show pyinstaller &>/dev/null; then
        pyinstaller build.spec --clean
    else
        echo -e "${YELLOW}[WARN]${RESET} PyInstaller no está instalado. Instalando temporalmente..."
        pip install pyinstaller
        pyinstaller build.spec --clean
    fi
}

ensure_ruff() {
    if ! command -v ruff &>/dev/null; then
        echo -e "\n${YELLOW}[WARN]${RESET} Ruff no está instalado en el entorno actual."
        read -rp "¿Deseas instalar 'ruff' en el entorno virtual actual? (s/n): " choice
        if [[ "$choice" =~ ^[sSyY]([iI][sS])?$ ]]; then
            echo -e "${GREEN}[INFO]${RESET} Instalando ruff..."
            pip install ruff
            return 0
        else
            echo -e "${YELLOW}[INFO]${RESET} Operación cancelada. Instala 'ruff' si deseas usar esta función."
            return 1
        fi
    fi
    return 0
}

bump_version_and_release() {
    echo -e "\n${CYAN}[BUMP VERSION]${RESET}"
    
    local CURR_VER="Desconocida"
    if [[ -f "build.spec" ]]; then
        CURR_VER=$(python -c "
import re
with open('build.spec', 'r') as f:
    c = f.read()
m = re.search(r\"version\s*=\s*'([^']+)'\", c)
print(m.group(1) if m else 'Desconocida')
" 2>/dev/null || echo "Desconocida")
    fi
    echo -e "Versión actual detectada en build.spec: ${BOLD}${CURR_VER}${RESET}"

    read -rp "Ingrese la nueva versión (ej. 1.0.1) [Cancelar si vacío]: " NEW_VER
    if [[ -z "$NEW_VER" ]]; then
        echo "Operación cancelada."
        return
    fi

    # Quitar 'v' o 'V' inicial
    if [[ "$NEW_VER" =~ ^[vV] ]]; then
        NEW_VER="${NEW_VER:1}"
    fi

    if [[ -f "build.spec" ]]; then
        python -c "
import re
with open('build.spec', 'r', encoding='utf-8') as f:
    c = f.read()
c = re.sub(r\"(version\s*=\s*')[^']+'\", r\"\g<1>${NEW_VER}'\", c)
c = re.sub(r\"('CFBundleShortVersionString'\s*:\s*')[^']+'\", r\"\g<1>${NEW_VER}'\", c)
c = re.sub(r\"('CFBundleVersion'\s*:\s*')[^']+'\", r\"\g<1>${NEW_VER}'\", c)
with open('build.spec', 'w', encoding='utf-8') as f:
    f.write(c)
"
        echo -e "${GREEN}[OK]${RESET} build.spec actualizado a la versión $NEW_VER"
    else
        echo -e "${RED}[ERROR]${RESET} No se encontró build.spec"
        return
    fi

    if [[ ! -d ".git" ]]; then
        echo -e "${YELLOW}[WARN]${RESET} No se detectó repositorio git. No se pueden realizar confirmaciones automáticas."
        return
    fi

    local CURRENT_BRANCH
    CURRENT_BRANCH=$(git branch --show-current)
    if [[ "$CURRENT_BRANCH" != "dev" ]]; then
        echo -e "${YELLOW}[WARN]${RESET} Se recomienda estar en la rama 'dev' para realizar la publicación."
    fi

    echo -e "\n${YELLOW}[WARN]${RESET} Se realizarán las siguientes acciones en git:"
    echo "  1. Confirmar cambios en '$CURRENT_BRANCH' (git commit)"
    echo "  2. Subir cambios a 'origin/$CURRENT_BRANCH' (git push)"
    echo "  3. Cambiar a la rama 'main' (git checkout main)"
    echo "  4. Fusionar '$CURRENT_BRANCH' en 'main' (git merge)"
    echo "  5. Crear etiqueta 'v$NEW_VER' (git tag)"
    echo "  6. Subir rama 'main' y etiqueta a origin (git push)"
    echo "  7. Regresar a la rama '$CURRENT_BRANCH' (git checkout)"

    read -rp "¿Confirmar publicación? (s/n): " CONFIRM
    if [[ "$CONFIRM" != "s" && "$CONFIRM" != "si" && "$CONFIRM" != "y" && "$CONFIRM" != "yes" ]]; then
        echo "Cancelado. Los archivos locales han sido modificados pero no confirmados en git."
        return
    fi

    git add build.spec
    git commit -m "Bump version to v$NEW_VER"
    git push origin "$CURRENT_BRANCH"

    echo -e "\n${GREEN}[INFO]${RESET} Cambiando a la rama 'main'..."
    git checkout main
    if [ $? -ne 0 ]; then
        echo -e "${RED}[ERROR]${RESET} No se pudo cambiar a la rama 'main'."
        return
    fi

    echo -e "${GREEN}[INFO]${RESET} Fusionando la rama '$CURRENT_BRANCH' en 'main'..."
    git merge "$CURRENT_BRANCH" --no-edit
    if [ $? -ne 0 ]; then
        echo -e "${RED}[ERROR]${RESET} Conflicto al fusionar '$CURRENT_BRANCH' en 'main'. Abortando fusión..."
        git merge --abort
        git checkout "$CURRENT_BRANCH"
        return
    fi

    echo -e "${GREEN}[INFO]${RESET} Creando etiqueta v$NEW_VER..."
    git tag "v$NEW_VER"

    echo -e "${GREEN}[INFO]${RESET} Subiendo rama 'main' y etiqueta v$NEW_VER al servidor remoto..."
    git push origin main
    git push origin "v$NEW_VER"

    echo -e "${GREEN}[INFO]${RESET} Regresando a la rama '$CURRENT_BRANCH'..."
    git checkout "$CURRENT_BRANCH"
    echo -e "\n${GREEN}[SUCCESS]${RESET} Versión v$NEW_VER publicada e insertada en Git."
}

run_and_pause() {
    if "$@"; then
        echo -e "\n${GREEN}[OK]${RESET} Comando completado con éxito."
    else
        echo -e "\n${RED}[FAILED]${RESET} El comando falló con el código de salida $?."
    fi
    read -rp "Presiona ENTER para volver al menú..." _
}

show_menu() {
    [[ -t 1 ]] && clear || true
    echo -e "${CYAN}${BOLD}==========================================${RESET}"
    echo -e "${CYAN}${BOLD}        Cardfile Manager Helper Shell     ${RESET}"
    echo -e "${CYAN}${BOLD}==========================================${RESET}"
    echo -e "  ${BOLD}1.${RESET} Ejecutar Cardfile (Escritorio)   ${YELLOW}(python main.py)${RESET}"
    echo -e "  ${BOLD}2.${RESET} Ejecutar Cardfile (Web)          ${YELLOW}(flet run --web main.py)${RESET}"
    echo -e "  ${BOLD}3.${RESET} Ejecutar pruebas unitarias       ${YELLOW}(unittest en /tests)${RESET}"
    echo -e "  ${BOLD}4.${RESET} Validación de sintaxis           ${YELLOW}(compileall)${RESET}"
    echo -e "  ${BOLD}5.${RESET} Análisis estático (Linter)       ${YELLOW}(ruff check)${RESET}"
    echo -e "  ${BOLD}6.${RESET} Verificación de formato          ${YELLOW}(ruff format --check)${RESET}"
    echo -e "  ${BOLD}7.${RESET} Limpiar directorio de build      ${YELLOW}(borrar build/ y dist/)${RESET}"
    echo -e "  ${BOLD}8.${RESET} Compilar ejecutable (PyInstaller)${YELLOW}(build.spec)${RESET}"
    echo -e "  ${BOLD}9.${RESET} Actualizar todos los paquetes    ${YELLOW}(pip upgrade)${RESET}"
    echo -e "  ${BOLD}10.${RESET} Actualizar requirements.txt     ${YELLOW}(pip freeze)${RESET}"
    echo -e "  ${BOLD}11.${RESET} Incrementar versión y publicar   ${YELLOW}(Git Tag & Push)${RESET}"
    echo -e "  ${BOLD}12.${RESET} Salir"
    echo -e "${CYAN}${BOLD}==========================================${RESET}"
}

# ---------------------------------------------------------------------------
# 3. Execution routing (checks for non-interactive / build flags)
# ---------------------------------------------------------------------------
if [[ "${1:-}" == "--build" ]]; then
    build_executable
    exit 0
fi

# Bucle interactivo
while true; do
    show_menu
    read -rp "Elige una opción (1-12): " opt

    case "$opt" in
        1)
            echo -e "\n${GREEN}[INFO]${RESET} Iniciando Cardfile en modo Escritorio...\n"
            run_and_pause python main.py
            ;;
        2)
            echo -e "\n${GREEN}[INFO]${RESET} Iniciando Cardfile en modo Web (Flet CLI)...\n"
            run_and_pause python -m flet run --web main.py
            ;;
        3)
            echo -e "\n${GREEN}[INFO]${RESET} Ejecutando pruebas unitarias...\n"
            PYTHONPATH=src:src/cardfile run_and_pause python -m unittest discover -s tests
            ;;
        4)
            echo -e "\n${GREEN}[INFO]${RESET} Validando sintaxis de archivos Python...\n"
            run_and_pause python -m compileall src tests
            ;;
        5)
            if ensure_ruff; then
                echo -e "\n${GREEN}[INFO]${RESET} Ejecutando ruff check...\n"
                run_and_pause ruff check src tests
            else
                read -rp "Presiona ENTER para volver al menú..." _
            fi
            ;;
        6)
            if ensure_ruff; then
                echo -e "\n${GREEN}[INFO]${RESET} Ejecutando ruff format --check...\n"
                run_and_pause ruff format --check src tests
            else
                read -rp "Presiona ENTER para volver al menú..." _
            fi
            ;;
        7)
            clean_build
            read -rp "Limpieza completada. Presiona ENTER para volver al menú..." _
            ;;
        8)
            build_executable
            read -rp "Presiona ENTER para volver al menú..." _
            ;;
        9)
            echo -e "\n${GREEN}[INFO]${RESET} Actualizando pip y todos los paquetes..."
            pip install --upgrade pip
            if pip install --upgrade -r requirements.txt pyinstaller ruff; then
                echo -e "\n${GREEN}[OK]${RESET} Paquetes actualizados correctamente."
            else
                echo -e "\n${RED}[FAILED]${RESET} Falló la actualización de paquetes."
            fi
            read -rp "Presiona ENTER para volver al menú..." _
            ;;
        10)
            echo -e "\n${GREEN}[INFO]${RESET} Regenerando requirements.txt a partir de pip freeze..."
            pip freeze > requirements.txt
            echo -e "${GREEN}[OK]${RESET} requirements.txt actualizado."
            read -rp "Presiona ENTER para volver al menú..." _
            ;;
        11)
            bump_version_and_release
            read -rp "Presiona ENTER para volver al menú..." _
            ;;
        12)
            echo -e "${CYAN}Bye!${RESET}"
            exit 0
            ;;
        *)
            echo -e "${YELLOW}[WARN]${RESET} Opción inválida. Elige entre 1 y 12."
            sleep 1
            ;;
    esac
done
