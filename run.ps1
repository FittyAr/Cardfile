# =============================================================================
#  Cardfile — Development & Test Helper PowerShell Script (Windows)
# =============================================================================

param (
    [switch]$Build
)

$env:NO_AT_BRIDGE = "1"

# Colors and formatting helpers
function Write-Color {
    param (
        [string]$Text,
        [string]$Color = "White",
        [bool]$NoNewLine = $false
    )
    if ($NoNewLine) {
        Write-Host $Text -ForegroundColor $Color -NoNewline
    } else {
        Write-Host $Text -ForegroundColor $Color
    }
}

# ---------------------------------------------------------------------------
# 1. Locate and activate virtual environment (.venv)
# ---------------------------------------------------------------------------
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

$IsInCI = ($env:GITHUB_ACTIONS -eq "true") -or ($env:CI -eq "true")

if (!$IsInCI) {
    if (!(Test-Path ".venv")) {
        Write-Color "[WARN] Entorno virtual .venv no encontrado." "Yellow"
        Write-Color "[INFO] Creando entorno virtual .venv..." "Green"
        
        # Try different python command names
        if (Get-Command "python" -ErrorAction SilentlyContinue) {
            python -m venv .venv
        } elseif (Get-Command "py" -ErrorAction SilentlyContinue) {
            py -m venv .venv
        } elseif (Get-Command "python3" -ErrorAction SilentlyContinue) {
            python3 -m venv .venv
        } else {
            Write-Color "[ERROR] Python no está instalado o no se encuentra en el PATH." "Red"
            Exit 1
        }
    }

    # Activar el entorno virtual
    $ActivateScript = Join-Path $ScriptDir ".venv\Scripts\Activate.ps1"
    if (Test-Path $ActivateScript) {
        . $ActivateScript
    } else {
        Write-Color "[ERROR] No se pudo encontrar el script de activación del entorno virtual." "Red"
        Exit 1
    }
}

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------
function Safe-RmTree {
    param ([string]$Path)
    if (Test-Path $Path) {
        Write-Color "[INFO] Eliminando $Path..." "Green"
        Remove-Item -Recurse -Force $Path -ErrorAction SilentlyContinue
    }
}

function Clean-Build {
    Safe-RmTree "build"
    Safe-RmTree "dist"
}

function Build-Executable {
    Clean-Build
    Write-Color "`n[INFO] Compilando ejecutable con PyInstaller..." "Green"
    
    # Check if pyinstaller is installed
    $PyInstCheck = pip show pyinstaller 2>$null
    if ($LASTEXITCODE -ne 0 -or !$PyInstCheck) {
        Write-Color "[WARN] PyInstaller no está instalado. Instalando temporalmente..." "Yellow"
        pip install pyinstaller
    }
    
    pyinstaller build.spec --clean
    if ($LASTEXITCODE -eq 0) {
        Write-Color "`n[OK] Compilación completada con éxito. Revisa la carpeta 'dist'" "Green"
        return $true
    } else {
        Write-Color "`n[FAILED] Falló la compilación. Código de salida: $LASTEXITCODE" "Red"
        return $false
    }
}

function Ensure-Ruff {
    $RuffCheck = Get-Command "ruff" -ErrorAction SilentlyContinue
    if (!$RuffCheck) {
        Write-Color "`n[WARN] Ruff no está instalado en el entorno actual." "Yellow"
        $Choice = Read-Host "¿Deseas instalar 'ruff' en el entorno virtual actual? (s/n)"
        if ($Choice -eq "s" -or $Choice -eq "si" -or $Choice -eq "y" -or $Choice -eq "yes") {
            Write-Color "[INFO] Instalando ruff..." "Green"
            pip install ruff
            return $true
        } else {
            Write-Color "[INFO] Operación cancelada. Instala 'ruff' si deseas usar esta función." "Yellow"
            return $false
        }
    }
    return $true
}

function Bump-Version-And-Release {
    Write-Color "`n[BUMP VERSION]" "Cyan"
    
    $CurrVer = "Desconocida"
    if (Test-Path "build.spec") {
        $Content = Get-Content "build.spec" -Raw
        if ($Content -match "version\s*=\s*'([^']+)'") {
            $CurrVer = $Matches[1]
        }
        if (!$CurrVer) { $CurrVer = "Desconocida" }
    }
    Write-Color "Versión actual detectada en build.spec: $CurrVer" "White"

    $NewVer = Read-Host "Ingrese la nueva versión (ej. 1.0.1) [Cancelar si vacío]"
    if ([string]::IsNullOrWhiteSpace($NewVer)) {
        Write-Color "Operación cancelada." "White"
        return
    }

    # Quitar 'v' o 'V' inicial
    if ($NewVer.StartsWith("v") -or $NewVer.StartsWith("V")) {
        $NewVer = $NewVer.Substring(1)
    }

    if (Test-Path "build.spec") {
        $Content = Get-Content "build.spec" -Raw -Encoding utf8
        $Content = $Content -replace "version\s*=\s*'[^']+'", "version='$NewVer'"
        $Content = $Content -replace "'CFBundleShortVersionString'\s*:\s*'[^']+'", "'CFBundleShortVersionString': '$NewVer'"
        $Content = $Content -replace "'CFBundleVersion'\s*:\s*'[^']+'", "'CFBundleVersion': '$NewVer'"
        Set-Content "build.spec" $Content -Encoding utf8
        Write-Color "[OK] build.spec actualizado a la versión $NewVer" "Green"
    } else {
        Write-Color "[ERROR] No se encontró build.spec" "Red"
        return
    }

    if (!(Test-Path ".git")) {
        Write-Color "[WARN] No se detectó repositorio git. No se pueden realizar confirmaciones automáticas." "Yellow"
        return
    }

    $CurrentBranch = git branch --show-current
    if ($CurrentBranch -ne "dev") {
        Write-Color "[WARN] Se recomienda estar en la rama 'dev' para realizar la publicación." "Yellow"
    }

    Write-Color "`n[WARN] Se realizarán las siguientes acciones en git:" "Yellow"
    Write-Color "  1. Confirmar cambios en '$CurrentBranch' (git commit)" "White"
    Write-Color "  2. Subir cambios a 'origin/$CurrentBranch' (git push)" "White"
    Write-Color "  3. Cambiar a la rama 'main' (git checkout main)" "White"
    Write-Color "  4. Fusionar '$CurrentBranch' en 'main' (git merge)" "White"
    Write-Color "  5. Crear etiqueta 'v$NewVer' (git tag)" "White"
    Write-Color "  6. Subir rama 'main' y etiqueta a origin (git push)" "White"
    Write-Color "  7. Regresar a la rama '$CurrentBranch' (git checkout)" "White"

    $Confirm = Read-Host "¿Confirmar publicación? (s/n)"
    if ($Confirm -ne "s" -and $Confirm -ne "si" -and $Confirm -ne "y" -and $Confirm -ne "yes") {
        Write-Color "Cancelado. Los archivos locales han sido modificados pero no confirmados en git." "White"
        return
    }

    git add build.spec
    git commit -m "Bump version to v$NewVer"
    git push origin $CurrentBranch

    Write-Color "`n[INFO] Cambiando a la rama 'main'..." "Green"
    git checkout main
    if ($LASTEXITCODE -ne 0) {
        Write-Color "[ERROR] No se pudo cambiar a la rama 'main'." "Red"
        return
    }

    Write-Color "[INFO] Fusionando la rama '$CurrentBranch' en 'main'..." "Green"
    git merge $CurrentBranch --no-edit
    if ($LASTEXITCODE -ne 0) {
        Write-Color "[ERROR] Conflicto al fusionar '$CurrentBranch' en 'main'. Abortando fusión..." "Red"
        git merge --abort
        git checkout $CurrentBranch
        return
    }

    Write-Color "[INFO] Creando etiqueta v$NewVer..." "Green"
    git tag "v$NewVer"

    Write-Color "[INFO] Subiendo rama 'main' y etiqueta v$NewVer al servidor remoto..." "Green"
    git push origin main
    git push origin "v$NewVer"

    Write-Color "[INFO] Regresando a la rama '$CurrentBranch'..." "Green"
    git checkout $CurrentBranch
    Write-Color "`n[SUCCESS] Versión v$NewVer publicada e insertada en Git." "Green"
}

function Run-And-Pause {
    param (
        [scriptblock]$ScriptBlock
    )
    
    # Run the script block
    & $ScriptBlock
    
    if ($LASTEXITCODE -eq 0) {
        Write-Color "`n[OK] Comando completado con éxito." "Green"
    } else {
        Write-Color "`n[FAILED] El comando falló con el código de salida $LASTEXITCODE." "Red"
    }
    
    Read-Host "Presiona ENTER para volver al menú..."
}

function Show-Menu {
    Clear-Host
    Write-Color "==========================================" "Cyan"
    Write-Color "        Cardfile Manager Helper PowerShell" "Cyan"
    Write-Color "==========================================" "Cyan"
    Write-Color "  1. Ejecutar Cardfile (Escritorio)   (python main.py)" "White"
    Write-Color "  2. Ejecutar Cardfile (Web)          (flet run --web main.py)" "White"
    Write-Color "  3. Ejecutar pruebas unitarias       (unittest en /tests)" "White"
    Write-Color "  4. Validación de sintaxis           (compileall)" "White"
    Write-Color "  5. Análisis estático (Linter)       (ruff check)" "White"
    Write-Color "  6. Verificación de formato          (ruff format --check)" "White"
    Write-Color "  7. Limpiar directorio de build      (borrar build/ y dist/)" "White"
    Write-Color "  8. Compilar ejecutable (PyInstaller)(build.spec)" "White"
    Write-Color "  9. Actualizar todos los paquetes    (pip upgrade)" "White"
    Write-Color "  10. Actualizar requirements.txt     (pip freeze)" "White"
    Write-Color "  11. Incrementar versión y publicar   (Git Tag & Push)" "White"
    Write-Color "  12. Salir" "White"
    Write-Color "==========================================" "Cyan"
}

# ---------------------------------------------------------------------------
# 3. Execution routing (checks for non-interactive / build flags)
# ---------------------------------------------------------------------------
if ($Build) {
    $Success = Build-Executable
    if ($Success) { Exit 0 } else { Exit 1 }
}

# Interactive loop
while ($true) {
    Show-Menu
    $Opt = Read-Host "Elige una opción (1-12)"
    
    switch ($Opt) {
        "1" {
            Write-Color "`n[INFO] Iniciando Cardfile en modo Escritorio...`n" "Green"
            Run-And-Pause { python main.py }
        }
        "2" {
            Write-Color "`n[INFO] Iniciando Cardfile en modo Web (Flet CLI)...`n" "Green"
            Run-And-Pause { python -m flet run --web main.py }
        }
        "3" {
            Write-Color "`n[INFO] Ejecutando pruebas unitarias...`n" "Green"
            $env:PYTHONPATH = "src"
            Run-And-Pause { python -m unittest discover -s tests }
            $env:PYTHONPATH = $null
        }
        "4" {
            Write-Color "`n[INFO] Validando sintaxis de archivos Python...`n" "Green"
            Run-And-Pause { python -m compileall src tests }
        }
        "5" {
            if (Ensure-Ruff) {
                Write-Color "`n[INFO] Ejecutando ruff check...`n" "Green"
                Run-And-Pause { ruff check src tests }
            } else {
                Read-Host "Presiona ENTER para volver al menú..."
            }
        }
        "6" {
            if (Ensure-Ruff) {
                Write-Color "`n[INFO] Ejecutando ruff format --check...`n" "Green"
                Run-And-Pause { ruff format --check src tests }
            } else {
                Read-Host "Presiona ENTER para volver al menú..."
            }
        }
        "7" {
            Clean-Build
            Read-Host "Limpieza completada. Presiona ENTER para volver al menú..."
        }
        "8" {
            $null = Build-Executable
            Read-Host "Presiona ENTER para volver al menú..."
        }
        "9" {
            Write-Color "`n[INFO] Actualizando pip y todos los paquetes..." "Green"
            python -m pip install --upgrade pip
            pip install --upgrade -r requirements.txt pyinstaller ruff
            if ($LASTEXITCODE -eq 0) {
                Write-Color "`n[OK] Paquetes actualizados correctamente." "Green"
            } else {
                Write-Color "`n[FAILED] Falló la actualización de paquetes." "Red"
            }
            Read-Host "Presiona ENTER para volver al menú..."
        }
        "10" {
            Write-Color "`n[INFO] Regenerando requirements.txt a partir de pip freeze..." "Green"
            pip freeze | Out-File -Encoding utf8 requirements.txt
            Write-Color "[OK] requirements.txt actualizado." "Green"
            Read-Host "Presiona ENTER para volver al menú..."
        }
        "11" {
            Bump-Version-And-Release
            Read-Host "Presiona ENTER para volver al menú..."
        }
        "12" {
            Write-Color "Bye!" "Cyan"
            Exit 0
        }
        default {
            Write-Color "[WARN] Opción inválida. Elige entre 1 y 12." "Yellow"
            Start-Sleep -Seconds 1
        }
    }
}
