# Cardfile 2.0 — Plan de reescritura en Rust + GPUI

> Estado: **aprobado** · Fecha: 2026-10-14 · Versión objetivo inicial: `2.0.0-alpha.1`

Este documento es la fuente de verdad del proyecto. `AGENTS.md` lo referencia y todo cambio de
alcance debe reflejarse aquí primero.

---

## 1. Decisiones tomadas

| # | Tema | Decisión |
|---|------|----------|
| 1 | Modo web / Docker | **Descartado**. Solo escritorio. |
| 2 | Autenticación | Configurable en el asistente de primer arranque: **multiusuario** (usuario + contraseña), **usuario único** (solo contraseña) o **sin contraseña**. |
| 3 | Bloqueo de tarjetas | Tres niveles, habilitables y elegibles por el usuario: **(A) Solo lectura** (evita ediciones accidentales, sin contraseña), **(B) Privada** (requiere contraseña para ver, título enmascarado), **(C) Cifrada** (contenido cifrado en reposo con clave derivada de contraseña). |
| 4 | Funcionalidades | Todas: etiquetas/categorías, imágenes embebidas, adjuntos, exportación (Markdown, HTML, PDF, JSON), favoritos/fijadas, orden manual, historial de versiones. |
| 5 | Búsqueda | Texto completo (título + contenido) con resaltado; alcance configurable (solo título / título + contenido / incluir etiquetas). |
| 6 | Papelera | Se mantiene (borrado lógico, restaurar, vaciar). |
| 7 | Importación | **No** existe ningún tipo de importación ni restauración (ni Flet, ni `.crd`, ni backups JSON). Se arranca de cero. |
| 8 | Temas | Se portan los 9 temas actuales, se crean nuevos y se soportan temas JSON externos. |
| 9 | Idiomas | Se desarrolla **solo en inglés**, con infraestructura i18n completa desde el día 1 (ningún string de UI hardcodeado). La traducción se hará al final. |
| 10 | Modo portable | Se mantiene. |
| 11 | Sincronización | Fuera de alcance, pero el modelo de datos queda **preparado** (UUIDv7, timestamps, revisión, tombstones). |
| 12 | Plataformas | Windows y Linux. **macOS queda fuera** por ahora (sin cuenta Apple Developer); el código se mantiene portable para sumarlo luego. |
| 13 | Atajos | Configurables mediante keymap JSON (mismo modelo que Zed). |
| 14–15 | Editor | WYSIWYG "lo más parecido a Word sin exagerar". El usuario nunca ve Markdown. Exportación a Markdown. |
| 16 | Atajos Markdown al escribir | Sí (`# ` → título, `- ` → lista, `[] ` → checklist, etc.). |
| 17 | Corrector ortográfico | Sí. |
| 18 | UI | **GPUI + gpui-component**. **No** se hace fork de Zed. |
| 19 | ORM | **Diesel** (SQLite). |
| 20 | Licencia | **`MIT OR Apache-2.0`**, copyright **FittyAr**. |
| 21 | Toolchain | Rust stable más reciente (1.99 al 2026-10-14), edición 2024. |
| 22 | Estructura | Workspace de Cargo con varios crates. |
| 23 | Limpieza | Se elimina todo el código Flet. Se conservan logos, imágenes y assets. El instalador Inno Setup se conserva y se actualiza. |
| 24 | Versión | `2.0.0-alpha.1`. |
| 25 | Idioma del repo | Código, identificadores y commits en inglés. Documentación en español e inglés. |
| 26 | Flujo Git | Conventional Commits, ramas + PR contra `main`. |
| 27 | Archivos de agente | Solo `AGENTS.md`. |
| 28 | Firma | Sin certificado pago. Microsoft Store firma el MSIX. Artefactos de release firmados con **minisign**. |
| 29 | Store / winget | Cuenta de Partner Center existente. ID winget: `FittyAr.Cardfile`. |
| 30 | Linux | Primero `.deb`, `.rpm`, AppImage; luego Flathub y AUR. |
| 31 | Actualizaciones | Aviso in-app de nueva versión (sin auto-instalar). Deshabilitado en builds de Store/winget/repos. |
| 32 | Telemetría | Ninguna. |
| 33 | Bloqueo por usuario | Cada usuario tiene su **contraseña de bloqueo** (por defecto, la de inicio de sesión; puede ser distinta) y un par de claves X25519. Cada tarjeta cifrada tiene una **clave aleatoria propia**, envuelta para cada usuario con acceso (formato v2). Cambiar la contraseña solo re-sella la clave secreta del usuario. Las tarjetas v1 se migran al desbloquearlas o al cambiar la contraseña. |
| 34 | Compartir | El dueño comparte una tarjeta con otros usuarios como **ver** o **editar** (modo multiusuario). Sección "Compartidas conmigo". Solo el dueño borra, protege, etiqueta o vuelve a compartir. Las cifradas se comparten envolviendo su clave para el destinatario. |
| 35 | Usuario IA y auditoría | Existe una cuenta `cardfile-ai` (tipo `ai`, sin login). Todo cambio guarda `updated_by`, el autor de cada revisión y una entrada en `activity` (actor, en nombre de quién, vía qué API key o "chat"). La IA ve exactamente lo que ve el usuario que le dio acceso. |
| 36 | MCP | Servidor MCP **HTTP (Streamable HTTP) dentro de la app**, solo en `127.0.0.1`, activable en Settings → IA, puerto configurable (7337). Autenticación con **API keys** por usuario (hash BLAKE3), con opciones *solo lectura* y *permitir privadas*; nunca cifradas. |
| 37 | Asistente IA | Presets para OpenAI, Anthropic, Gemini, Mistral, Groq, DeepSeek, xAI, OpenRouter, Together, Perplexity, Azure OpenAI, Ollama, LM Studio y personalizados. Panel de chat con herramientas (buscar, leer, crear, reescribir, editar texto, etiquetar, favoritos/fijadas, reordenar, papelera). Aplica cambios directo: revisión previa en el historial y borrados a la papelera. Toda la UI de IA se oculta hasta configurar un servicio. |
| 38 | Multimedia | Reproducción inline solo de audio libre de patentes: MP3, WAV, OGG/Vorbis y FLAC. El video, el AAC/M4A y los demás formatos se abren con el reproductor del sistema; si ninguna aplicación puede abrirlos, la app se lo informa al usuario. Sin códecs con patentes (H.264, AAC). |
| 39 | Adjuntos | Imágenes y adjuntos se arrastran dentro de la tarjeta (mover; Ctrl/Alt copia) o sobre otra tarjeta de la lista (mover; Ctrl/Alt copia). |
| 40 | Escrituras | CJK en UI y PDF (fuente de respaldo del sistema). RTL: el editor reordena los glifos según el algoritmo bidi Unicode y alinea a la derecha los párrafos RTL; etiquetas de UI en orden visual (con formas de presentación árabes); HTML con `dir="rtl"`. Límite: los inputs de una línea de gpui-component siguen mostrando RTL de izquierda a derecha. |
| 41 | Traducciones | 21 idiomas, todos salvo el inglés hechos con IA; los README lo indican y piden reportes. |
| 42 | Barra de título | Propia, al estilo de Zed: decoraciones del cliente (`TitleBar` de gpui-component), menú de la app, ícono y título; controles de ventana dibujados por la app en Windows y Linux. |
| 43 | Tipografía | **Inter** (texto e interfaz) y **JetBrains Mono** (código), OFL 1.1, incrustadas en el binario (`cardfile-doc::fonts`) y usadas también en el PDF. CJK/árabe/hebreo usan fuentes del sistema como respaldo. |
| 44 | Logo | Rediseño plano (fichas apiladas con pestaña, paleta coral) en SVG (`assets/brand/`); todos los PNG/ICO se generan con `scripts/render_brand.py`. |
| 45 | Claves de IA | Las API keys de los servicios de IA se guardan en el almacén de credenciales del sistema (`keyring`: Credential Manager / Secret Service); si no está disponible, en la base con aviso. |
| 46 | Configuración | Diálogo amplio con barra lateral agrupada (App, Privacidad y seguridad, IA, Datos); la IA se divide en Servicios, Asistente y Apps de IA (MCP). Alta de servicios guiada: elegir proveedor (nube/local) → API key → modelo → nombre; opciones avanzadas plegadas. |
| 47 | Imágenes | Redimensionables con manijas: esquina mantiene proporción; bordes derecho/inferior estiran libremente (`Image.height`). |
| 48 | Campos de una línea | El título de la ficha y la entrada del chat usan el editor propio en modo una línea, que dispone el texto RTL correctamente. |

### 1.1 ¿Por qué no un fork de Zed?

- **Licencia**: GPUI es Apache-2.0, pero la mayoría de los crates de Zed (`editor`, `workspace`,
  `project`, …) son **GPL-3.0 / AGPL-3.0**. Un fork obligaría a licenciar Cardfile como GPL.
- **Tamaño**: Zed tiene 200+ crates (LSP, colaboración, terminal, IA, extensiones). Recortarlo y
  mantenerlo sincronizado cuesta más que construir sobre GPUI.
- **gpui-component** (Longbridge, Apache-2.0) ya aporta 60+ widgets, temas, dock, render de
  Markdown/HTML y un input de texto basado en `ropey`.

**Regla**: nunca copiar código de crates GPL/AGPL de Zed. Solo se depende de `gpui`
(publicado como `gpui-pre`) y crates con licencia permisiva.

---

## 2. Stack tecnológico

Las versiones se fijan con `cargo add` (última disponible) al comenzar cada fase y se
registran en `Cargo.lock`. Valores verificados al 2026-10-14:

| Área | Crate / herramienta | Versión ref. | Licencia |
|------|---------------------|--------------|----------|
| UI framework + widgets | `gpui-kit` (agrupa `gpui-pre` 0.3.9 + gpui-component + assets) | 0.7.3 | Apache-2.0 |
| Buffer de texto | `ropey` (vía gpui-component) | 2.0.0-beta | MIT |
| ORM | `diesel` + `diesel_migrations` (feature `sqlite`, SQLite bundled) | 2.3.x | MIT OR Apache-2.0 |
| Búsqueda | SQLite **FTS5** (tabla virtual vía migraciones SQL) | — | Dominio público |
| IDs | `uuid` (v7) | última | MIT OR Apache-2.0 |
| Fechas | `jiff` o `time` | última | MIT OR Apache-2.0 |
| Serialización | `serde`, `serde_json` | última | MIT OR Apache-2.0 |
| Errores | `thiserror` (libs), `anyhow` (bin) | última | MIT OR Apache-2.0 |
| Logs | `tracing`, `tracing-subscriber`, `tracing-appender` | última | MIT |
| Hash de contraseñas | `argon2` (RustCrypto) | última | MIT OR Apache-2.0 |
| Cifrado | `chacha20poly1305` (XChaCha20-Poly1305), `zeroize`, `rand` | última | MIT OR Apache-2.0 |
| Hash de blobs | `blake3` | última | CC0 / Apache-2.0 |
| Markdown (export) | `pulldown-cmark` / `comrak` (GFM) | última | MIT / BSD-2 |
| PDF (export) | `krilla` + `harfrust` (medición; fuentes del sistema) | última | MIT OR Apache-2.0 / MIT |
| Resaltado de código | `syntect` + `two-face` (vía gpui-component) | última | MIT |
| Imágenes | `image` | última | MIT OR Apache-2.0 |
| Corrector | `spellbook` (Hunspell-compatible) + diccionario en_US de `wooorm/dictionaries` | última | MPL-2.0* / MIT-like |
| Audio (adjuntos) | `rodio` (salida vía `cpal`, sin decoders propios) + `symphonia` (MP3, WAV, OGG/Vorbis, FLAC; sin AAC ni MP4) | 0.23.0 / 0.6.1 | MIT OR Apache-2.0 / MPL-2.0* |
| i18n | `rust-i18n` | última | MIT |
| Directorios | `directories` | última | MIT OR Apache-2.0 |
| Config | `serde` + TOML (`toml`) | última | MIT OR Apache-2.0 |
| Updater | `ureq` + `semver` + `minisign-verify` + `sha2`; `zip`, `tar`, `lzma-rust2` (desempaquetado, Rust puro) | última | MIT / Apache / MIT OR Apache |
| Diálogos de archivo | `rfd` (si GPUI no expone lo necesario) | última | MIT |
| Releases | `dist` (cargo-dist) | 0.32.x | MIT OR Apache-2.0 |
| Paquetes Linux | `cargo-deb`, `cargo-generate-rpm`, `linuxdeploy` (AppImage) | última | MIT |
| Licencias | `cargo-deny`, `cargo-about` | última | MIT OR Apache-2.0 |
| Changelog | `git-cliff` | última | MIT OR Apache-2.0 |

\* **MPL-2.0** es copyleft débil a nivel de archivo: compatible con distribuir Cardfile bajo
MIT/Apache siempre que no modifiquemos los archivos de `spellbook`. Se aprueba como excepción
explícita en `deny.toml`; si se prefiere evitarlo, alternativa: implementación propia sobre
listas de palabras (más pobre). Lo mismo aplica a los crates de `symphonia` (decodificación de
audio en `cardfile-media`), usados sin modificar. El video, el AAC/M4A y cualquier otro formato
quedan fuera de alcance (se evitan los códecs con patentes): la UI los abre con el reproductor del
sistema. Nunca ffmpeg, GStreamer ni libvlc.

### 2.1 Política de licencias

- `deny.toml` con allowlist: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`,
  `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`, `CC0-1.0`, `MPL-2.0` (solo crates listados y sin modificar:
  `spellbook`; `symphonia`, `symphonia-core`, `symphonia-common`, `symphonia-metadata`,
  `symphonia-bundle-flac`, `symphonia-bundle-mp3`, `symphonia-codec-pcm`,
  `symphonia-codec-vorbis`, `symphonia-format-ogg`, `symphonia-format-riff` [decodificación de audio, `cardfile-media`]; y transitivas `cbindgen`
  [build], `dwrote` [GPUI], `option-ext` [`directories`]).
  **GPL/AGPL/LGPL prohibidas.** `cargo deny check` corre en CI y bloquea el merge.
- `cargo about generate` produce `THIRD_PARTY_LICENSES.html` en cada release y se incluye en los
  paquetes y en la pantalla "About → Licenses" de la app.
- `docs/CREDITS.md` cita proyectos de los que tomamos ideas, assets o código (con enlace y licencia).
- Todo código copiado de terceros conserva su cabecera de licencia original.

---

## 3. Arquitectura

### 3.1 Workspace

```
Cardfile/
├─ Cargo.toml                 # [workspace], dependencias compartidas, perfiles
├─ rust-toolchain.toml        # channel = "stable"
├─ crates/
│  ├─ cardfile/               # bin: entrypoint, wiring, CLI args (--portable, --data-dir)
│  ├─ cardfile-core/          # dominio puro: entidades, servicios, errores, traits de repos
│  ├─ cardfile-db/            # Diesel: schema.rs, migraciones, impl de repositorios
│  ├─ cardfile-crypto/        # argon2, cifrado de tarjetas, manejo de claves en memoria
│  ├─ cardfile-doc/           # modelo de documento + exportadores (md, html, pdf, json)
│  ├─ cardfile-editor/        # widget GPUI del editor WYSIWYG
│  ├─ cardfile-ui/            # vistas GPUI: shell, lista, wizard, settings, diálogos
│  ├─ cardfile-i18n/          # carga de locales (en.yml) + macro t!()
│  ├─ cardfile-media/         # reproducción inline de audio MP3/WAV/OGG/FLAC (sin GPUI)
│  ├─ cardfile-ai/            # cliente LLM bloqueante: presets, formatos de API, tool calling
│  ├─ cardfile-mcp/           # servidor MCP Streamable HTTP (tiny_http), herramientas vía trait
│  ├─ cardfile-agent/         # herramientas de la IA sobre la DB (compartidas por MCP y chat)
│  └─ cardfile-updater/       # chequeo de versión + verificación minisign
├─ assets/                    # iconos, logos, temas JSON, keymaps, fuentes, diccionarios
├─ locales/en.yml
├─ packaging/
│  ├─ windows/installer.iss   # Inno Setup actualizado
│  ├─ windows/msix/           # AppxManifest.xml + assets de Store
│  └─ linux/                  # .desktop, metainfo AppStream, iconos, AppImage, flatpak
├─ docs/
├─ scripts/                   # render_brand.py (iconos desde SVG), check_locales.py, …
└─ .github/workflows/
```

Dependencias permitidas entre crates (sin ciclos):
`core` ← `db`, `crypto`, `doc` ← `editor` ← `ui` ← `cardfile` (bin). `core` no depende de GPUI ni de Diesel.

### 3.2 Modelo de documento (lo que guarda el editor)

Como el editor tiene capacidades tipo Word (colores, alineación) que Markdown no expresa,
el formato canónico es un **árbol de bloques JSON versionado** (`doc_version`), similar a
ProseMirror/Notion:

- **Bloques**: `paragraph`, `heading(1..3)`, `bullet_list`, `ordered_list`, `check_list`,
  `quote`, `code_block(lang)`, `table`, `image(blob_id, width)`, `attachment(blob_id)`,
  `divider`, `callout`.
- **Atributos de bloque**: alineación (izq/centro/der/justificado), sangría.
- **Marcas inline**: bold, italic, underline, strike, code, link, color de texto, resaltado,
  tamaño (pequeño/normal/grande/muy grande — escala fija, no libre), super/subíndice.
- **Exportación**: Markdown (GFM; lo no expresable se degrada o usa HTML inline opcional),
  HTML, PDF, JSON (copia fiel).
- Se guarda también `plain_text` derivado para FTS5 y vista previa en la lista.

### 3.3 Base de datos (Diesel + SQLite)

Preparada para sincronización futura: PK `UUIDv7` (texto/blob), `created_at`, `updated_at`,
`deleted_at` (tombstone), `rev` (contador), `device_id`.

| Tabla | Campos principales |
|-------|-------------------|
| `users` | id, username, password_hash (argon2id, null si sin contraseña), display_name, is_admin, settings_json, created/updated/deleted |
| `cards` | id, owner_id, title, content_json, plain_text, lock_mode (`none`/`read_only`/`private`/`encrypted`), encrypted_payload, crypto_meta, is_pinned, is_favorite, sort_key (fraccional), created/updated/deleted, rev |
| `card_revisions` | id, card_id, content_json (o cifrado), title, created_at — historial con límite configurable |
| `tags` | id, owner_id, name, color |
| `card_tags` | card_id, tag_id |
| `blobs` | id (blake3), mime, size, data (BLOB) — imágenes y adjuntos, deduplicados |
| `card_blobs` | card_id, blob_id |
| `app_settings` | key, value_json — config global (modo de auth, etc.) |
| `cards_fts` | tabla virtual FTS5 (title, plain_text) mantenida con triggers |

- Migraciones embebidas (`embed_migrations!`) aplicadas al arrancar, con backup automático previo.
- Acceso a DB en hilo de fondo (`cx.background_executor()`), nunca en el hilo de UI.
- Pragmas: `journal_mode=WAL`, `foreign_keys=ON`, `busy_timeout`.
- Las tarjetas cifradas no se indexan en FTS (solo título si no es privada).
- Migración `2026-10-15-000000_ai_sharing`: `users.kind/lock_hash/public_key/secret_key`,
  `cards.updated_by`, `card_revisions.author_id`, tablas `card_shares`, `card_keys`, `api_keys`
  y `activity`.

### 3.4 Autenticación y bloqueo

- **Modo de auth** se elige en el asistente de primer arranque (para las builds de Store no hay
  forma práctica de preguntar en el instalador, por eso es en el primer arranque).
  Modificable luego por un admin en Settings.
  - *Multiusuario*: pantalla de login; el primer usuario es admin y gestiona usuarios.
  - *Usuario único*: pantalla de solo contraseña.
  - *Sin contraseña*: entra directo.
- Bloqueo automático por inactividad (configurable) y al minimizar (opcional).
- **Niveles de bloqueo por tarjeta**:
  - *Solo lectura*: el editor queda inmutable; un clic en el candado (o atajo) desbloquea.
  - *Privada*: título enmascarado, contenido oculto hasta ingresar contraseña; re-bloqueo temporal.
  - *Cifrada*: contenido con XChaCha20-Poly1305 bajo una clave aleatoria por tarjeta, envuelta
    (X25519 + BLAKE3 + XChaCha20) para cada usuario con acceso; la clave secreta del usuario se
    sella con Argon2id bajo su contraseña de bloqueo. Claves en memoria con `zeroize`.
    Advertencia clara: contraseña perdida = datos perdidos.
- Settings permite habilitar/deshabilitar cada nivel y elegir el nivel por defecto.

### 3.5 Editor WYSIWYG (`cardfile-editor`) — módulo de mayor riesgo

Componente propio sobre primitivas de texto de GPUI (`TextLayout`, `ShapedLine`,
`InputHandler` para IME), sin código GPL de Zed:

- Modelo: árbol de bloques (3.2) + selección/cursor por (bloque, offset). Cada bloque usa un `ropey`.
- Operaciones como transacciones → **undo/redo** con agrupación.
- Barra de herramientas tipo Word (compacta) + menú contextual + atajos (`Ctrl+B/I/U`, etc.).
- Atajos Markdown al escribir (input rules) y "slash menu" (`/` para insertar bloque).
- Pegado: texto plano, HTML (convertido al modelo), imágenes del portapapeles.
- Drag & drop de imágenes y archivos.
- Tablas: insertar/quitar filas/columnas, navegación con Tab.
- Corrector ortográfico con subrayado y sugerencias en menú contextual.
- IME y accesibilidad: soporte de IME desde el inicio; accesibilidad según lo que exponga GPUI.

Se valida con un **spike temprano** (Fase 1) antes de construir el resto de la UI sobre él.

### 3.6 UI

- Layout: sidebar (búsqueda, filtros por etiqueta, favoritos, papelera) + lista de tarjetas +
  panel de tarjeta (editor). Ventana redimensionable, estado persistido.
- Componentes de gpui-component (List, Input, Dialog, Popover, Sidebar, Dock, Theme).
- Temas: los 9 actuales (snow, midnight, cyber, forest, nordic, sunset, lavender, ocean,
  graphite) + nuevos (p. ej. "paper", "solarized light/dark", "high contrast"), en JSON en
  `assets/themes/`, más carpeta de temas del usuario.
- Keymap por defecto en `assets/keymaps/default.json`, override en el directorio de config.
- Command palette (`Ctrl+Shift+P`).

### 3.7 Datos y configuración en disco

- Estándar: carpeta `FittyAr/Cardfile`, compartida por todas las apps FittyAr: Windows
  `%APPDATA%\FittyAr\Cardfile\data` (`directories::ProjectDirs`), Linux
  `$XDG_DATA_HOME/FittyAr/Cardfile` (`~/.local/share` si no está), macOS
  `~/Library/Application Support/FittyAr/Cardfile`. La carpeta anterior (`$XDG_DATA_HOME/cardfile`,
  `ar.FittyAr.Cardfile` en macOS) se renombra al arrancar: nunca se borra ni se mezcla, y si no se
  puede mover se sigue usando.
- Portable: si existe `portable.flag` (o `--portable`) junto al ejecutable, o ya hay un
  `data/cardfile.db` junto a él (carpeta copiada o sincronizada, p. ej. en OneDrive), todo va en `./data`.
- Archivos: `cardfile.db` (incluye toda la configuración: global en `app_settings`, preferencias en
  cada usuario), `keymap.json`, `themes/`, `logs/`, `backups/`. No hay `settings.toml`.

---

## 4. Convenciones (resumen; detalle en `AGENTS.md`)

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check` deben pasar.
- Sin `unwrap()`/`expect()` fuera de tests salvo invariantes documentadas.
- Errores tipados por crate (`thiserror`); `anyhow` solo en el bin.
- Todo string visible pasa por `t!()`; claves en `locales/en.yml`.
- Tests: unitarios en cada crate, integración de DB con SQLite en memoria, tests de
  ida y vuelta del modelo de documento y exportadores, tests de UI con `gpui::TestAppContext`.
- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`, `ci:`, `build:`).
  Ramas `feat/...`, `fix/...`, `chore/...`; PR contra `main`, squash merge, título del PR = commit.

---

## 5. CI/CD y distribución

### 5.1 Workflows

| Workflow | Disparador | Jobs |
|----------|-----------|------|
| `ci.yml` | PR y push a `main` | fmt, clippy, test (matriz Windows/Linux), `cargo deny`, título de PR semántico |
| `release.yml` | tag `v*` | generado/ampliado desde `dist`: build por target, archivos, instaladores shell/PowerShell, Inno Setup, MSIX, `.deb`, `.rpm`, AppImage, `SHA256SUMS`, firma minisign, licencias de terceros, GitHub Release con changelog `git-cliff` |
| `winget.yml` | llamado por `release.yml` (post-announce job, con la release ya publicada) | `winget-releaser` → PR a `microsoft/winget-pkgs` para `FittyAr.Cardfile` (requiere PAT `WINGET_TOKEN`; la primera versión se envía a mano) |
| `store.yml` | manual / release estable | sube el MSIX a Partner Center (`msstore` CLI; secretos de Azure AD de Partner Center) |

Targets: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`. (macOS se agregará cuando exista cuenta Apple Developer.)

### 5.2 Instalación con un comando

```powershell
irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1 | iex
```
```bash
curl -fsSL https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.sh | sh
```
```powershell
winget install FittyAr.Cardfile
```

`install.ps1` e `install.sh` (raíz del repo, servidos desde `main`) no están atados a una versión:
consultan la API de GitHub e instalan la release más nueva (estable, o la pre-release más nueva si
no hay una estable posterior; `-Stable`/`--stable` y `-Version`/`--version` lo cambian), verifican
el SHA-256 contra el `SHA256SUMS` firmado y las firmas minisign (minisign oficial descargado y
fijado por SHA-256 si no está instalado), y crean los accesos directos:
- **Windows**: `%LOCALAPPDATA%\Programs\FittyAr\Cardfile` (carpeta FittyAr compartida por todos los
  proyectos), accesos en menú Inicio y escritorio, PATH de
  usuario y entrada en *Aplicaciones instaladas* (desinstalador `uninstall.ps1` local).
- **Linux**: `~/.local/lib/FittyAr/Cardfile` + enlace `~/.local/bin/cardfile`, `.desktop` en
  `~/.local/share/applications`, íconos hicolor y acceso en el escritorio (`xdg-user-dir`).
Migran la instalación anterior de cargo-dist (`~/.cardfile/bin`). Inspirado en el instalador de
Pairee (mismo autor).

### 5.3 Firma

- **minisign**: clave secreta en el secreto `MINISIGN_SECRET_KEY` (+ password). Se firman todos los
  artefactos y `SHA256SUMS`. La clave pública va en el README, en `docs/` y embebida en el updater.
- **Windows**: el MSIX lo firma Microsoft Store. Los `.exe`/instalador Inno quedan sin firma
  Authenticode (SmartScreen avisará; documentado). Opción futura: Azure Trusted Signing.
- **macOS**: fuera de alcance por ahora.

### 5.4 Updater

Consulta las releases de GitHub al arrancar (configurable, desactivable), compara `semver`
(pre-releases solo si la versión instalada es pre-release) y solo anuncia releases cuyo
`SHA256SUMS` lleva una firma minisign válida de la clave embebida (`PUBLIC_KEY`).

Actualización automática (`cardfile-updater`, sin GPUI, funciones bloqueantes en el executor de
fondo), al estilo de Zed (solo ideas; nada de su código GPL):

1. **Tipo de instalación** (`InstallKind::detect`): Store/MSIX (`WindowsApps`), gestor de
   paquetes Linux (`/usr`, salvo `/usr/local`), Flatpak (`FLATPAK_ID`), Snap (`SNAP`), AppImage
   (`APPIMAGE`), Inno Setup (`unins000.exe` junto al exe), portable (`portable.flag`) o binario
   suelto en carpeta escribible (`install.ps1`/`install.sh`, archivo descomprimido).
   Store, gestores de paquetes, Flatpak, Snap y copias en carpetas de solo lectura **no** se
   autoactualizan: siguen con el aviso y el enlace a la release.
2. **Asset**: `.zip` (Windows) o `.tar.xz` (Linux) para binario suelto/portable, el `.AppImage`
   de la arquitectura, o el `-setup-<arch>.exe` de Inno Setup.
3. **Descarga** con progreso y cancelación a un temporal junto al destino (mismo sistema de
   archivos). Se exigen **ambas** verificaciones: SHA-256 contra el `SHA256SUMS` firmado y la
   firma `.minisig` propia del asset (comentario de confianza con tag y nombre de archivo).
   Cualquier diferencia cancela la instalación.
4. **Instalación**: archivo → se desempaqueta junto al exe y cada archivo incluido reemplaza al
   instalado (en Windows el exe en uso se renombra a `cardfile.exe.old`, borrado en el siguiente
   arranque por `cleanup_after_update`; en Linux, rename atómico). AppImage → se reemplaza el
   archivo (temporal + chmod + rename). Inno Setup → se ejecuta el instalador con
   `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH` y la app sale; el
   instalador la vuelve a abrir (`[Run]` con `/RELAUNCH`).
5. **Reinicio**: `restart(exe)` lanza el nuevo ejecutable con los mismos argumentos y la app sale.

Feature `update-check` en `cardfile-ui` para desactivarlo en builds de tienda/paquete.

---

## 6. Fases

Cada fase termina con PR(s) mergeados, CI verde y criterios de aceptación cumplidos.

### Fase 0 — Limpieza y andamiaje
1. Rama `chore/rust-rewrite-bootstrap`.
2. Eliminar código Flet: `src/`, `tests/`, `docker/`, `lang/`, `main.py`, `requirements.txt`,
   `run.ps1`, `run.sh`, `build.spec`, docs obsoletas (`ANALISIS.md`, `MIGRACION.md`,
   `auth_redirect_fix.md`, `markdown_editor_*.md`, `compilacion_y_release.md`, `signing.md`,
   READMEs traducidos), workflow `release.yml` actual.
3. Conservar `assets/` y mover `installer.iss` → `packaging/windows/installer.iss` (se actualiza en Fase 8;
   se mantiene el `AppId` GUID original).
4. Crear: `.gitignore` (Rust + IDE + OS), `.gitattributes`, `.editorconfig`, `rust-toolchain.toml`,
   `rustfmt.toml`, `clippy.toml`, `deny.toml`, `about.toml`, `cliff.toml`, `LICENSE-MIT`,
   `LICENSE-APACHE` (FittyAr), `README.md` + `README.es.md`, `AGENTS.md`, `CONTRIBUTING.md`,
   `docs/CREDITS.md`, `.github/` (ci.yml, plantillas de PR/issue, dependabot para cargo y actions).
5. Workspace con todos los crates vacíos; el bin abre una ventana GPUI "Hello Cardfile".
- **Aceptación**: `cargo build` en Windows y Linux en CI, `cargo deny` verde.

### Fase 1 — Spike del editor (riesgo técnico)
- Prototipo en `cardfile-editor`: párrafos + headings + bold/italic, cursor, selección con
  mouse/teclado, IME, undo/redo, copy/paste de texto plano, varios miles de palabras sin lag.
- **Aceptación**: demo funcional en `examples/` y documento corto con decisiones/límites.
  Si el enfoque no escala, se replantea aquí, no después.

### Fase 2 — Núcleo de datos
- `cardfile-core` (entidades, servicios, traits), `cardfile-db` (schema, migraciones, repos, FTS5,
  triggers, backups), `cardfile-doc` (modelo + serialización + `plain_text`).
- **Aceptación**: tests de repos y de búsqueda; ida y vuelta JSON del documento.

### Fase 3 — Shell de la aplicación
- Ventana, sidebar, lista de tarjetas, crear/renombrar/borrar/restaurar, papelera, búsqueda
  configurable con resaltado, temas (9 + nuevos + externos), keymap, command palette,
  Settings (General/Apariencia/Editor/Seguridad/Datos/Acerca de), i18n, modo portable, logs.
- **Aceptación**: CRUD completo con editor básico de Fase 1.

### Fase 4 — Seguridad
- Asistente de primer arranque (idioma, almacenamiento estándar/portable, modo de auth, tema).
- Login multiusuario / contraseña única / sin contraseña; gestión de usuarios (admin).
- Bloqueo por inactividad; niveles de tarjeta Solo lectura / Privada / Cifrada; cambio de contraseña
  con re-cifrado.
- **Aceptación**: tests de crypto (vectores, contraseña incorrecta, corrupción), flujos de UI.

### Fase 5 — Editor completo
- Listas (todas), checklists, citas, divisor, bloques de código con resaltado, tablas, links,
  colores, resaltado, tamaños, alineación, sangría, imágenes (pegar/arrastrar/redimensionar),
  adjuntos, atajos Markdown, slash menu, pegado HTML, buscar/reemplazar dentro de la tarjeta.
- **Aceptación**: suite de tests del modelo y de input rules; prueba manual guiada.

### Fase 6 — Organización y exportación
- Etiquetas con color, filtros, favoritos, fijadas, orden manual (drag & drop), historial de
  versiones con vista/restauración, exportación de tarjeta o colección a Markdown/HTML/PDF/JSON.
- **Aceptación**: exportaciones validadas con fixtures (golden files).

### Fase 7 — Corrector ortográfico
- `spellbook` + diccionario en_US, subrayado, sugerencias, "agregar al diccionario", activar/desactivar,
  diccionarios por idioma preparados para la traducción.

### Fase 8 — Empaquetado y distribución
- `dist` + jobs extra: Inno Setup actualizado (branding actual, idiomas),
  MSIX + Store, `.deb`/`.rpm`/AppImage, minisign, `THIRD_PARTY_LICENSES`, winget, updater.
- **Aceptación**: tag `v2.0.0-alpha.1` publica una release completa e instalable por cada vía.

### Fase 9 — Pulido y estable
- Rendimiento (10k tarjetas), accesibilidad, docs de usuario (es/en), `CREDITS.md` completo,
  entrega de `locales/en.yml` para traducción, Flathub y AUR, `2.0.0`.

---

## 7. Riesgos

| Riesgo | Mitigación |
|--------|-----------|
| GPUI es pre-1.0 y su API cambia | Pin exacto igual al de gpui-component; actualizar juntos; aislar GPUI en `editor`/`ui`. |
| Editor WYSIWYG es complejo | Spike en Fase 1; alcance "Word sin exagerar"; modelo propio simple. |
| Sin firma Authenticode / Apple | Store para Windows firmado, minisign + documentación; evaluar Azure Trusted Signing. |
| Dependencias Linux de GPUI (Vulkan, Wayland/X11) | Documentar requisitos; AppImage/Flatpak empaquetan lo necesario. |
| MPL-2.0 del corrector | Excepción explícita y sin modificar el crate; alternativa documentada. |
| Pérdida de datos cifrados | Advertencias, backups automáticos, tests exhaustivos. |

---

## 8. Puntos resueltos

1. No hay importación ni restauración de ningún tipo.
2. Se acepta la excepción MPL-2.0 para `spellbook`.
3. macOS queda fuera hasta contar con cuenta Apple Developer.

---

## 9. Estado (2026-10-14)

| Fase | Estado | PR |
|------|--------|----|
| 0 — Limpieza y andamiaje | ✅ | #1 |
| 1 — Spike del editor | ✅ (validado con tests de interacción GPUI) | #2 |
| 2 — Núcleo de datos | ✅ | #2 |
| 3 — Shell de la aplicación | ✅ | #3 |
| 4 — Seguridad | ✅ | #3, #4 |
| 5 — Editor completo | ✅ (buscar/reemplazar y menú `/` en #4) | #2, #4 |
| 6 — Organización y exportación | ✅ | #3 |
| 7 — Corrector ortográfico | ✅ (diccionarios descargados en CI) | #3 |
| 8 — Empaquetado y distribución | ✅ configurado; se valida con el primer tag | #3 |
| 9 — Pulido y estable | 🔶 en curso | — |
| 10 — IA, compartir y multimedia (`2.0.0-alpha.2`) | ✅ | — |

Fase 10 (decisiones 33–41): bloqueo por usuario con claves por tarjeta, compartir tarjetas,
usuario IA con auditoría, servidor MCP con API keys, asistente IA con presets, reproducción de
audio/video, arrastre de adjuntos, CJK/RTL y 20 traducciones. Corregido el cierre inesperado al
insertar un link (doble préstamo de la entidad del editor).

Revisión de los PR #1–#4 (incluida en #4): imágenes y adjuntos de tarjetas cifradas sellados con
una clave por blob guardada dentro del documento cifrado (`sealed:<id>:<clave>`); el updater solo
anuncia releases con `SHA256SUMS` firmado por la clave minisign incrustada; `cards_fts` enlazado
por `card_id`; búsqueda sin mayúsculas con plegado Unicode; Argon2, diccionarios y lecturas del
login fuera del hilo de UI.

Pendiente:

- Primer release `v2.0.0-alpha.1`: secreto y variable minisign ya configurados; falta `WINGET_TOKEN`
  (`docs/RELEASING.md`), crear el tag y revisar
  los artefactos generados por el pipeline.
- Primera publicación manual en winget (`wingetcreate new`) y en Microsoft Store (Partner Center).
- Traducciones: `locales/en.yml` está completo; copiar a `locales/<lang>.yml` para cada idioma.
- Flathub y AUR.
- Pegado de HTML enriquecido: GPUI expone el portapapeles solo como texto, imagen o rutas; hoy se
  pega texto plano o Markdown.
- Rendimiento con decenas de miles de tarjetas: contadores con `COUNT(*)` en vez de listar.
- macOS cuando exista cuenta Apple Developer.
