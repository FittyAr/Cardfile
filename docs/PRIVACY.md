# Privacy policy

*Last updated: 2026-11-22* · [Español](#política-de-privacidad)

NoteChest is a desktop app. Your cards stay on your computer. The developer does not
run any server that receives your notes, and the app has no telemetry, analytics,
ads or crash reporting.

## What stays on your computer

- **Cards, tags, attachments and history** are stored in a local SQLite database
  (`%APPDATA%\FittyAr\NoteChest` on Windows, `~/.local/share/FittyAr/NoteChest` on
  Linux, or a `data` folder next to the app in portable mode). Backups are written
  to the same place, or to a folder you choose.
- **Passwords** are never stored. NoteChest keeps only an Argon2id hash. Encrypted
  cards are encrypted on your computer before they are saved.
- **AI service API keys** are kept in your system's credential manager. If none is
  available, they are kept in the NoteChest database.

## When NoteChest connects to the internet

NoteChest only connects in these cases:

| Feature | Destination | What is sent | When |
|---------|-------------|--------------|------|
| Update check | `api.github.com` and `github.com` (FittyAr/NoteChest releases) | A standard request for the release list. No personal data. | Builds downloaded from GitHub only. It is off in the Microsoft Store and in Linux package-manager builds, which update through the store. |
| Spell-check dictionaries | `raw.githubusercontent.com` | A request for the dictionary file. | Only when you download a dictionary. |
| AI assistant | The AI service **you** configure (for example OpenAI, Azure OpenAI, OpenRouter, DeepSeek, Perplexity, or a local Ollama or LM Studio) | Your messages and the card content the assistant reads to answer you. | Only when you use the assistant. The service's own privacy policy applies. Encrypted cards are never sent; private cards only if you allow it in *Settings → AI*. |
| MCP server | `127.0.0.1` only (your own computer) | Card data, to the AI client you connect. | Only if you turn it on and give a client an API key. Other computers cannot reach it. |
| Links | Your web browser | Nothing beyond opening the page. | When you click "User guide", "Report a problem" and similar links. |

## AI-generated content

Assistant answers are generated live by the AI service you choose. They can be wrong
or inappropriate. Use **Report** in the assistant panel, or open an issue at
<https://github.com/FittyAr/NoteChest/issues>, to report a harmful answer. A report
contains only what you write in it.

## Your choices

- Delete cards, empty the recycle bin, or delete the data folder to remove your data.
  Uninstalling the app does not delete your notes.
- Remove AI keys in *Settings → AI*. Turn off the MCP server in *Settings*.

## Contact

Questions about this policy: <https://github.com/FittyAr/NoteChest/issues>.

---

# Política de privacidad

*Última actualización: 2026-11-22*

NoteChest es una app de escritorio. Tus tarjetas se quedan en tu computadora. El
desarrollador no tiene ningún servidor que reciba tus notas, y la app no tiene
telemetría, analíticas, publicidad ni reporte de errores automático.

## Qué se queda en tu computadora

- **Tarjetas, etiquetas, adjuntos e historial** se guardan en una base SQLite local
  (`%APPDATA%\FittyAr\NoteChest` en Windows, `~/.local/share/FittyAr/NoteChest` en
  Linux, o una carpeta `data` junto a la app en modo portable). Los respaldos se
  escriben ahí mismo o en la carpeta que elijas.
- **Las contraseñas** nunca se guardan; solo un hash Argon2id. Las tarjetas cifradas
  se cifran en tu computadora antes de guardarse.
- **Las claves de API de servicios de IA** se guardan en el administrador de
  credenciales del sistema. Si no hay uno disponible, se guardan en la base de NoteChest.

## Cuándo se conecta a internet

NoteChest solo se conecta en estos casos:

| Función | Destino | Qué se envía | Cuándo |
|---------|---------|--------------|--------|
| Buscar actualizaciones | `api.github.com` y `github.com` (releases de FittyAr/NoteChest) | Una consulta estándar de la lista de versiones, sin datos personales. | Solo en las versiones descargadas de GitHub. Está desactivada en la Microsoft Store y en los paquetes de Linux, que se actualizan por su tienda. |
| Diccionarios del corrector | `raw.githubusercontent.com` | La descarga del diccionario. | Solo cuando descargas un diccionario. |
| Asistente de IA | El servicio de IA que **tú** configures (por ejemplo OpenAI, Azure OpenAI, OpenRouter, DeepSeek, Perplexity, o un Ollama o LM Studio local) | Tus mensajes y el contenido de las tarjetas que el asistente lee para responder. | Solo cuando usas el asistente. Rige la política de privacidad de ese servicio. Las tarjetas cifradas nunca se envían; las privadas solo si lo permites en *Configuración → IA*. |
| Servidor MCP | Solo `127.0.0.1` (tu propia computadora) | Datos de tarjetas, al cliente de IA que conectes. | Solo si lo activas y le das una clave de API a un cliente. Otras computadoras no pueden acceder. |
| Enlaces | Tu navegador | Nada más que abrir la página. | Al hacer clic en "Guía de usuario", "Reportar un problema" y enlaces similares. |

## Contenido generado por IA

Las respuestas del asistente las genera en el momento el servicio de IA que elijas, y
pueden ser incorrectas o inapropiadas. Usa **Reportar** en el panel del asistente, o
abre un issue en <https://github.com/FittyAr/NoteChest/issues>, para reportar una
respuesta dañina. El reporte contiene solo lo que escribas en él.

## Tus opciones

- Borra tarjetas, vacía la papelera o elimina la carpeta de datos para quitar tus datos.
  Desinstalar la app no borra tus notas.
- Quita las claves de IA en *Configuración → IA*. Desactiva el servidor MCP en
  *Configuración*.

## Contacto

Preguntas sobre esta política: <https://github.com/FittyAr/NoteChest/issues>.
