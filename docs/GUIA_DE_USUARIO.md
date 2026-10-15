# Guía de usuario de Cardfile

English: [USER_GUIDE.md](USER_GUIDE.md)

> La interfaz de Cardfile 2.0 está por ahora en inglés. Los nombres de menús aparecen en inglés
> entre paréntesis.

## Primer arranque

La primera vez que abrís Cardfile, un asistente te pregunta cuatro cosas:

1. **Almacenamiento.**
   - *Estándar* guarda las tarjetas en tu perfil de usuario.
   - *Portable* las guarda en una carpeta `data` junto a Cardfile, ideal para un pendrive.
2. **Inicio de sesión.**
   - *Sin contraseña*: Cardfile abre directo.
   - *Contraseña*: una cuenta protegida con contraseña.
   - *Varios usuarios*: cada persona entra con su usuario y contraseña, y tiene sus propias tarjetas.
3. **Protección de tarjetas.** Qué niveles de protección se pueden usar (ver [Proteger tarjetas](#proteger-tarjetas)).
4. **Tema.**

Todo esto se puede cambiar después en **Configuración (Settings)**.

## Tarjetas

- **Nueva tarjeta:** el botón **+** o `Ctrl+N`. Escribí el título arriba y presioná `Enter` para pasar al texto.
- Las tarjetas se **guardan solas** mientras escribís.
- **Fijá (Pin)** una tarjeta para que quede arriba en la lista. Marcala con la **estrella** para sumarla a *Favoritos*.
- **Reordenar:** arrastrá las tarjetas en la lista. Requiere el orden *Manual* (botón de orden sobre la lista).
- **Borrar:** menú **⋯** o `Ctrl+Shift+Backspace`. Las tarjetas van a la **Papelera (Recycle bin)**, desde donde podés restaurarlas o borrarlas para siempre.

## Escribir

El editor funciona como un procesador de texto. Usá la barra de herramientas o estos atajos:

| Acción | Atajo |
|--------|-------|
| Negrita, cursiva, subrayado | `Ctrl+B`, `Ctrl+I`, `Ctrl+U` |
| Tachado, código en línea | `Ctrl+Shift+X`, ``Ctrl+` `` |
| Títulos 1–3, texto normal | `Ctrl+Alt+1`…`3`, `Ctrl+Alt+0` |
| Viñetas, numeración, checklist | `Ctrl+Shift+8`, `Ctrl+Shift+7`, `Ctrl+Shift+9` |
| Alinear izquierda, centro, derecha, justificar | `Ctrl+L`, `Ctrl+E`, `Ctrl+R`, `Ctrl+J` |
| Sangría | `Tab` / `Shift+Tab` en listas, o `Ctrl+]` / `Ctrl+[` |
| Vínculo | `Ctrl+K` |
| Salto de línea dentro del párrafo | `Shift+Enter` |
| Buscar / reemplazar en la tarjeta | `Ctrl+F` / `Ctrl+H`, y `F3` para la siguiente coincidencia |
| Deshacer / rehacer | `Ctrl+Z` / `Ctrl+Y` |
| Quitar formato | `Ctrl+Espacio` |

### Atajos al escribir

Si estás acostumbrado a Markdown, esto se convierte en formato mientras escribís (Configuración → Editor → *Markdown shortcuts*):

- Al inicio de una línea:
  - `# `, `## `, `### ` → título
  - `- ` o `* ` → viñetas
  - `1. ` → numeración
  - `[] ` → checklist
  - `> ` → cita
- Solos en una línea, seguidos de `Enter`:
  - ```` ``` ```` → bloque de código
  - `---` → separador
- Alrededor de palabras: `**negrita**`, `*cursiva*`, `` `código` ``, `~~tachado~~`

Escribí `/` en una línea vacía para abrir un menú de bloques: títulos, listas, tabla, imagen, separador…

### Imágenes, archivos y tablas

- **Pegá** una imagen o **arrastrá** imágenes y archivos a la tarjeta. Las imágenes aparecen en el texto y los demás archivos quedan como adjuntos. Hacé doble clic en un adjunto para abrirlo.
- **Insert → Table** crea una tabla de 3×3.
  - `Tab` pasa a la celda siguiente; en la última celda agrega una fila.
  - Con clic derecho sobre la tabla podés agregar o quitar filas y columnas.
- Hacé clic en una imagen para ver sus manijas: arrastrá la **esquina** para cambiar el tamaño manteniendo la proporción, o el borde **derecho** o **inferior** para estirarla. Con clic derecho podés achicarla, agrandarla, ajustarla al ancho o recuperar su proporción.
- **Arrastrá una imagen o un adjunto** para moverlo dentro de la tarjeta. Soltalo sobre otra tarjeta de la lista para moverlo ahí; mantené `Ctrl` (o `Alt`) al soltar para copiarlo.
- **El audio se reproduce en la tarjeta.** El audio MP3, WAV, OGG y FLAC tiene un reproductor con play/pausa, barra de avance y silencio. El video, el AAC/M4A y los demás formatos se abren con el reproductor del sistema; si ninguna aplicación de tu computadora puede abrirlos, Cardfile te avisa.

### Otros idiomas y escrituras

El chino, el japonés y el coreano funcionan en todos lados, también al exportar a PDF (Cardfile usa una fuente CJK instalada en el sistema). El texto de derecha a izquierda (hebreo, árabe, persa) se muestra en el orden correcto y esos párrafos se alinean a la derecha. Límite conocido: los cuadros de texto de una línea (como el título de la tarjeta mientras lo editás) todavía muestran ese texto de izquierda a derecha.

### Ortografía

Las palabras mal escritas aparecen subrayadas. Con clic derecho ves sugerencias o podés **agregarla al diccionario**. Se activa, se desactiva y se elige el diccionario en Configuración → Editor.

## Buscar y organizar

- `Ctrl+F` (fuera del editor) busca en todas las tarjetas.
- Elegí dónde buscar en Configuración → Editor → *Search in*: solo títulos, títulos y contenido, o también etiquetas. Las coincidencias se resaltan en la lista.
- **Etiquetas:** se crean con el **+** junto a *TAGS* en la barra lateral, o con el botón de etiqueta de una tarjeta. Hacé clic en una etiqueta de la barra lateral para ver sus tarjetas. Con clic derecho sobre una etiqueta podés renombrarla, cambiarle el color o borrarla.
- **Historial de versiones:** menú **⋯** → *Version history* (`Ctrl+Shift+H`) muestra versiones anteriores de la tarjeta, y podés restaurar cualquiera. Mientras editás se guarda como máximo una versión cada 5 minutos.

## Proteger tarjetas

Usá el botón del candado arriba de la tarjeta.

| Nivel | Qué hace |
|-------|----------|
| **Solo lectura** | Podés leer la tarjeta, pero tenés que hacer clic en **Edit** antes de modificarla. Evita ediciones por error. |
| **Privada** | El título aparece enmascarado en la lista y el contenido queda oculto hasta que ingresás tu contraseña. |
| **Cifrada** | El contenido se guarda cifrado con tu contraseña (Argon2id + XChaCha20-Poly1305). **Si olvidás la contraseña, el contenido no se puede recuperar.** |

Las tarjetas privadas y cifradas se vuelven a bloquear después de un rato sin actividad (Configuración → Seguridad).

En el modo *Sin contraseña*, configurá una **contraseña de bloqueo** en Configuración → Seguridad antes de usar tarjetas privadas o cifradas.

**Cada usuario tiene su propia contraseña de bloqueo.** Por defecto es tu contraseña de inicio de sesión; en Configuración → Seguridad podés poner otra distinta (si escribís tu contraseña de inicio como nueva contraseña de bloqueo, volvés a usar una sola). Cambiarla no vuelve a cifrar tus tarjetas: cada tarjeta cifrada tiene su propia clave aleatoria, protegida por separado para cada persona que puede abrirla.

Con contraseña o con varios usuarios, Cardfile también puede **bloquearse solo** después de un tiempo de inactividad. Para bloquearlo en el momento, usá `Ctrl+Shift+L`.

## Compartir tarjetas

Con varios usuarios (modo multiusuario), abrí el menú **⋯** de una tarjeta → *Compartir…* (o el botón de personas) y elegí para cada usuario **Sin acceso**, **Puede ver** o **Puede editar**. Las tarjetas compartidas aparecen en **Compartidas conmigo** en la barra lateral. Solo el dueño puede borrar, proteger, etiquetar o compartir una tarjeta. Quienes pueden editar ven en la barra de estado y en el historial quién hizo el último cambio.

Para compartir una tarjeta **cifrada**, desbloqueala primero. La otra persona la abre con *su propia* contraseña de bloqueo; necesita haber iniciado sesión (o desbloqueado una tarjeta protegida) al menos una vez para que Cardfile cree sus claves.

## Asistente de IA

Cardfile puede trabajar con una IA en la nube o en tu computadora. No aparece nada relacionado con IA hasta que la configures.

1. Abrí **Configuración → IA → Agregar servicio de IA** y elegí un preset: OpenAI, Anthropic (Claude), Google Gemini, Mistral, Groq, DeepSeek, xAI, OpenRouter, Together AI, Perplexity, Azure OpenAI, **Ollama** o **LM Studio** (locales), o *Personalizado* para cualquier servicio compatible.
2. Ingresá tu API key (los servicios locales no la necesitan), elegí un modelo y tocá **Probar conexión**.
3. El **botón del asistente** (ícono de robot arriba de la lista, o `Ctrl+Shift+A`) abre el panel de chat.

Pedile cosas con tus palabras: buscar tarjetas, reescribir o dar formato a una tarjeta, convertir notas en una lista de tareas, poner etiquetas, marcar favoritas, reordenar, mandar tarjetas a la papelera… Cada cambio queda registrado como hecho por **Cardfile AI** en tu nombre, la versión anterior queda en el historial y lo borrado va a la papelera, así que siempre podés deshacer. El asistente nunca ve tarjetas cifradas; las privadas solo si lo permitís en Configuración → IA.

## Usar Cardfile desde otras apps de IA (MCP)

Cardfile incluye un servidor MCP para que apps como Claude Desktop, Claude Code, Cursor o VS Code usen tus tarjetas mientras Cardfile está abierto.

1. En **Configuración → IA**, activá **Ejecutar el servidor MCP** (escucha solo en `127.0.0.1`; puerto por defecto 7337).
2. En **API keys**, creá una clave. Elegí *Solo lectura* o *Permitir tarjetas privadas* si querés. Copiala: se muestra una sola vez, junto con la configuración lista para pegar en cada app.
3. La app de IA ve exactamente lo que ves vos. Sus cambios quedan registrados como hechos por Cardfile AI a través de esa clave. **Revocá** una clave para cortar el acceso al instante. La lista muestra cuándo se usó cada clave por última vez y *Actividad reciente de la IA* muestra qué cambió.

## Actualizaciones

Cardfile busca versiones nuevas al iniciar (Configuración → General). Cuando hay una, hacé clic en el aviso, mirá las novedades y elegí **Actualizar y reiniciar**: Cardfile la descarga, verifica su firma y la instala. Las copias instaladas desde Microsoft Store o un gestor de paquetes de Linux se actualizan por esa vía.

## Exportar y respaldar

- **Una tarjeta:** menú **⋯** → Markdown, HTML, PDF o JSON.
- **Todas las tarjetas:** Configuración → Datos → Export.
  - Las tarjetas privadas y cifradas no se exportan.
  - Las exportaciones a Markdown guardan las imágenes en una carpeta `blobs`.
- **Respaldo:** menú de configuración → *Back up now* guarda una copia de la base de datos en la carpeta `backups`. También se hace un respaldo automático antes de cada actualización de la base de datos.

## Atajos de teclado y comandos

- `Ctrl+Shift+P` abre la **paleta de comandos**, con todos los comandos.
- Para cambiar cualquier atajo, abrí Configuración → General → *Keyboard shortcuts*. Se abre `keymap.json`, que lista todos los nombres de acciones. Reiniciá Cardfile para aplicar los cambios.

## ¿Dónde están mis archivos?

Abrí el menú de configuración y elegí **Open data folder**. Salvo que uses una de las opciones de abajo, es:

- Windows: `%APPDATA%\FittyAr\Cardfile\data`
- Linux: `~/.local/share/FittyAr/Cardfile` (`$XDG_DATA_HOME/FittyAr/Cardfile` si está definida)
- macOS: `~/Library/Application Support/FittyAr/Cardfile`

Las versiones anteriores usaban `~/.local/share/cardfile` en Linux y `~/Library/Application Support/ar.FittyAr.Cardfile` en macOS; Cardfile mueve esa carpeta al lugar nuevo la primera vez que arranca. Si no puede moverla, la sigue usando.

La carpeta contiene:

- `cardfile.db`: todas tus tarjetas
- `backups/`: respaldos de la base de datos
- `logs/`: registros, útiles para reportar errores
- `themes/`: temas adicionales, en formato JSON de gpui-component
- `keymap.json`: tus atajos

Opciones de línea de comandos:

- `cardfile --portable` guarda los datos junto al ejecutable.
- `cardfile --data-dir <carpeta>` usa la carpeta que elijas.
