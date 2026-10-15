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
- Compartir tarjetas con otros usuarios (ver o editar); cada usuario tiene su propia contraseña
  de bloqueo y sus claves.
- Asistente de IA opcional con tu propia IA en la nube o local (OpenAI, Claude, Gemini, Ollama,
  LM Studio y más): buscá, reescribí, formateá, etiquetá y reordená tarjetas conversando. No se
  muestra hasta que lo configurás.
- Servidor MCP integrado y API keys, para que apps de IA como Claude o Cursor usen tus tarjetas;
  cada cambio de la IA queda registrado a su nombre y se puede deshacer.
- El audio adjunto se reproduce dentro de la tarjeta; el video se abre con el reproductor del
  sistema. Arrastrá imágenes y archivos entre tarjetas.
- Chino, japonés, coreano y escrituras de derecha a izquierda (hebreo, árabe, persa).
- Interfaz en 21 idiomas.
- Sin telemetría.

> **Sobre las traducciones:** todas las traducciones distintas del inglés fueron hechas con IA.
> Pedimos disculpas por los errores que puedan tener y agradecemos los reportes para corregirlos:
> [abrí un issue](https://github.com/FittyAr/Cardfile/issues).

## Instalación

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1 | iex
```

**Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.sh | sh
```

Los instaladores de una línea siempre instalan la release **más nueva** (estable, o la
pre-release más nueva mientras no haya una estable más reciente), verifican su SHA-256 y su firma
minisign, y agregan Cardfile al menú Inicio / menú de aplicaciones y al escritorio. Todo queda dentro de una carpeta `FittyAr` (`%LOCALAPPDATA%\Programs\FittyAr\Cardfile`,
`~/.local/lib/FittyAr/Cardfile`). No hacen falta
permisos de administrador y después Cardfile se actualiza solo desde la app. Opciones:

| | Windows | Linux |
|---|---|---|
| Solo releases estables | `-Stable` | `--stable` |
| Una versión concreta | `-Version v2.0.0` | `--version v2.0.0` |
| Sin acceso en el escritorio | `-NoDesktopShortcut` | `--no-desktop-shortcut` |
| Desinstalar | `-Uninstall` (o *Configuración → Aplicaciones*) | `--uninstall` |

En Windows las opciones se pasan con
`& ([scriptblock]::Create((irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1))) -Stable`;
en Linux con `curl … | sh -s -- --stable`.

**Windows (winget)**

```powershell
winget install FittyAr.Cardfile
```

Cada [release](https://github.com/FittyAr/Cardfile/releases) también incluye `.deb`, `.rpm`,
AppImage y el instalador de Inno Setup. Microsoft Store y los repositorios de Linux están en
camino.

## Documentación

- [Guía de usuario](docs/GUIA_DE_USUARIO.md)

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
