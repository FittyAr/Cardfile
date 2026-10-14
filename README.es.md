<p align="center"><img src="assets/logo_trasparente.png" alt="Cardfile" width="160"></p>

# Cardfile

Un fichero de tarjetas digital y moderno para tus notas, inspirado en el clásico Cardfile de
Windows 3.1. Es nativo, rápido y privado: está escrito en Rust con [GPUI](https://gpui.rs), el
framework de UI del editor [Zed](https://zed.dev).

> **Estado:** Cardfile 2 es una reescritura completa y está en **alpha**. English: [README.md](README.md)

## Funciones

- Editor tipo Word: títulos, listas, checklists, tablas, bloques de código, colores, imágenes y
  adjuntos. No hace falta aprender Markdown, y cada tarjeta se puede exportar a Markdown.
- Búsqueda de texto completo en títulos y contenido.
- Etiquetas, favoritos, tarjetas fijadas, orden manual e historial de versiones.
- Niveles de protección por tarjeta: **solo lectura** (evita ediciones por error), **privada**
  (pide contraseña para verla) y **cifrada** (se guarda cifrada).
- Modo multiusuario, de contraseña única o sin contraseña, elegido en el primer arranque.
- Papelera, temas, atajos de teclado configurables y paleta de comandos.
- Exportación a Markdown, HTML, PDF y JSON.
- Modo portable, para llevar tus tarjetas en un pendrive.
- Sin telemetría.

## Instalación

**Windows (PowerShell)**

```powershell
irm https://github.com/FittyAr/Cardfile/releases/latest/download/cardfile-installer.ps1 | iex
```

**Windows (winget)**

```powershell
winget install FittyAr.Cardfile
```

**Linux**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/FittyAr/Cardfile/releases/latest/download/cardfile-installer.sh | sh
```

Cada [release](https://github.com/FittyAr/Cardfile/releases) también incluye `.deb`, `.rpm`,
AppImage y el instalador de Inno Setup. Microsoft Store y los repositorios de Linux están en
camino.

## Compilar desde el código fuente

Necesitás la última versión estable de [Rust](https://rustup.rs).

```bash
git clone https://github.com/FittyAr/Cardfile
cd Cardfile
cargo run --release
```

## Licencia

Podés usarlo bajo [Apache 2.0](LICENSE-APACHE) o [MIT](LICENSE-MIT), a tu elección. Los créditos
de terceros están en [docs/CREDITS.md](docs/CREDITS.md).
