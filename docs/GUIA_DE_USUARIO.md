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
- Con clic derecho sobre una imagen podés achicarla, agrandarla o ajustarla al ancho.

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

Con contraseña o con varios usuarios, Cardfile también puede **bloquearse solo** después de un tiempo de inactividad. Para bloquearlo en el momento, usá `Ctrl+Shift+L`.

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

Abrí el menú de configuración y elegí **Open data folder**. La carpeta contiene:

- `cardfile.db`: todas tus tarjetas
- `backups/`: respaldos de la base de datos
- `logs/`: registros, útiles para reportar errores
- `themes/`: temas adicionales, en formato JSON de gpui-component
- `keymap.json`: tus atajos

Opciones de línea de comandos:

- `cardfile --portable` guarda los datos junto al ejecutable.
- `cardfile --data-dir <carpeta>` usa la carpeta que elijas.
