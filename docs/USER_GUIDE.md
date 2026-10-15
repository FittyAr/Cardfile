# Cardfile user guide

Español: [GUIA_DE_USUARIO.md](GUIA_DE_USUARIO.md)

## First run

The first time you start Cardfile, a short wizard asks four things:

1. **Storage.**
   - *Standard* keeps your cards in your user profile.
   - *Portable* keeps them in a `data` folder next to Cardfile, which is handy on a USB drive.
2. **Sign-in.**
   - *No password*: Cardfile opens directly.
   - *Password*: one account, protected by a password.
   - *Several users*: everyone signs in with their own name and password and has their own cards.
3. **Card protection.** Choose which protection levels can be used (see [Protecting cards](#protecting-cards)).
4. **Theme.**

You can change all of this later in **Settings**.

## Cards

- **New card:** the **+** button, or `Ctrl+N`. Type the title at the top. Press `Enter` to jump into the text.
- Cards are **saved automatically** while you type.
- **Pin** a card to keep it at the top of the list. **Star** it to add it to *Favorites*.
- **Reorder:** drag cards in the list. This needs the *Manual* sort order (the sort button above the list).
- **Delete:** the **⋯** menu or `Ctrl+Shift+Backspace`. Deleted cards go to the **Recycle bin**, where you can restore them or delete them permanently.

## Writing

The editor works like a word processor. Use the toolbar or these shortcuts:

| Action | Shortcut |
|--------|----------|
| Bold, italic, underline | `Ctrl+B`, `Ctrl+I`, `Ctrl+U` |
| Strikethrough, inline code | `Ctrl+Shift+X`, ``Ctrl+` `` |
| Headings 1–3, normal text | `Ctrl+Alt+1`…`3`, `Ctrl+Alt+0` |
| Bulleted, numbered, checklist | `Ctrl+Shift+8`, `Ctrl+Shift+7`, `Ctrl+Shift+9` |
| Align left, center, right, justify | `Ctrl+L`, `Ctrl+E`, `Ctrl+R`, `Ctrl+J` |
| Indent / outdent | `Tab` / `Shift+Tab` in lists, or `Ctrl+]` / `Ctrl+[` |
| Link | `Ctrl+K` |
| Line break inside a paragraph | `Shift+Enter` |
| Find / replace in the card | `Ctrl+F` / `Ctrl+H`, then `F3` for the next match |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Clear formatting | `Ctrl+Space` |

### Shortcuts while typing

If you prefer Markdown habits, these turn into formatting as you type (Settings → Editor → *Markdown shortcuts*):

- At the start of a line:
  - `# `, `## `, `### ` → heading
  - `- ` or `* ` → bulleted list
  - `1. ` → numbered list
  - `[] ` → checklist
  - `> ` → quote
- On a line of its own, followed by `Enter`:
  - ```` ``` ```` → code block
  - `---` → divider
- Around words: `**bold**`, `*italic*`, `` `code` ``, `~~strike~~`

Type `/` in an empty line to open a menu of blocks: headings, lists, table, image, divider…

### Images, files and tables

- **Paste** an image, or **drag** images and files into a card. Images appear in the text; other files become attachments. Double-click an attachment to open it.
- **Insert → Table** creates a 3×3 table.
  - `Tab` moves to the next cell; in the last cell it adds a row.
  - Right-click a table to add or remove rows and columns.
- Click an image to show its handles: drag the **corner** to resize it keeping its proportions, or the **right** or **bottom** edge to stretch it. Right-click it to make it smaller, larger, fit the width or restore its proportions.
- **Drag an image or attachment** to move it inside the card. Drop it on another card in the list to move it there; hold `Ctrl` (or `Alt`) while dropping to copy it instead.
- **Audio plays inside the card.** MP3, WAV, OGG and FLAC audio get a player with play/pause, a seek bar and mute. Video, AAC/M4A and other formats open in your system player; if no application on your computer can open them, Cardfile tells you.

### Other languages and scripts

Chinese, Japanese and Korean text works everywhere, including PDF export (Cardfile uses a CJK font installed on your system). Right-to-left text (Hebrew, Arabic, Persian) is shown in the right order and right-to-left paragraphs are aligned to the right. Known limit: one-line text boxes (such as the card title while you edit it) still show right-to-left text left to right.

### Spelling

Misspelled words are underlined. Right-click one for suggestions or **Add to dictionary**. You can turn this on or off and choose the dictionary in Settings → Editor.

## Searching and organizing

- `Ctrl+F` (outside the editor) searches every card.
- Choose where to search in Settings → Editor → *Search in*: titles only, titles and content, or also tags. Matches are highlighted in the list.
- **Tags:** create them with the **+** next to *TAGS* in the sidebar, or with the tag button on a card. Click a tag in the sidebar to see its cards. Right-click a tag to rename it, change its color or delete it.
- **Version history:** the **⋯** menu → *Version history* (`Ctrl+Shift+H`) shows earlier versions of a card. You can restore any of them. A version is kept at most every 5 minutes while you edit.

## Protecting cards

Use the lock button at the top of a card.

| Level | What it does |
|-------|--------------|
| **Read-only** | You can read the card, but you must click **Edit** before changing it. This prevents accidental edits. |
| **Private** | The title is masked in the list and the content is hidden until you enter your password. |
| **Encrypted** | The content is encrypted on disk with your password (Argon2id + XChaCha20-Poly1305). **If you forget the password, the content cannot be recovered.** |

Private and encrypted cards lock again after a while without activity (Settings → Security).

In *No password* mode, set a **lock password** in Settings → Security before using private or encrypted cards.

**Every user has their own lock password.** By default it is your sign-in password; in Settings → Security you can set a different one (type your sign-in password as the new lock password to go back to one password). Changing it does not re-encrypt your cards: each encrypted card has its own random key, locked separately for every person who can open it.

With a password or several users, Cardfile can also **lock itself** after a period of inactivity. Lock it now with `Ctrl+Shift+L`.

## Sharing cards

With several users (multi-user mode), open a card's **⋯** menu → *Share…* (or the people button) and choose, for each user, **No access**, **Can view** or **Can edit**. Shared cards appear under **Shared with me** in the sidebar. Only the owner can delete, protect, tag or share a card. People who can edit see who made the last change in the status bar and in the version history.

To share an **encrypted** card, unlock it first. The other person opens it with *their own* lock password; they need to have signed in (or unlocked a protected card) once so that Cardfile created their keys.

## AI assistant

Cardfile can work with an AI, in the cloud or on your computer. Nothing AI-related appears until you set it up.

1. Open **Settings → AI → Add AI service** and pick a preset: OpenAI, Anthropic (Claude), Google Gemini, Mistral, Groq, DeepSeek, xAI, OpenRouter, Together AI, Perplexity, Azure OpenAI, **Ollama** or **LM Studio** (local), or *Custom* for any compatible service.
2. Enter your API key (local services don't need one), choose a model and press **Test connection**.
3. The **assistant button** (robot icon above the card list, or `Ctrl+Shift+A`) opens the chat panel.

Ask in your own words: find cards, rewrite or format a card, turn notes into a checklist, add tags, mark favorites, reorder cards, move cards to the recycle bin… Every change is recorded as made by **Cardfile AI** on your behalf, the previous version is kept in the history, and deleted cards go to the recycle bin, so you can always undo. The assistant never sees encrypted cards; private cards only if you allow it in Settings → AI.

## Using Cardfile from other AI apps (MCP)

Cardfile includes an MCP server so AI apps such as Claude Desktop, Claude Code, Cursor or VS Code can use your cards while Cardfile is open.

1. In **Settings → AI**, turn on **Run the MCP server** (it listens on `127.0.0.1` only; default port 7337).
2. Under **API keys**, create a key. Choose *Read only* or *Allow private cards* if you want. Copy the key: it is shown only once, together with ready-to-paste settings for each AI app.
3. The AI app sees exactly what you see. Its changes are recorded as made by Cardfile AI, through that key. **Revoke** a key to cut its access immediately. The list shows when each key was last used, and *Recent AI activity* shows what the AI changed.

## Updates

Cardfile checks for new versions at startup (Settings → General). When one is available, click the notice, read what's new and choose **Update and restart**: Cardfile downloads it, checks its signature and installs it. Copies installed from the Microsoft Store or a Linux package manager are updated by them instead.

## Export and backup

- **One card:** the **⋯** menu → Markdown, HTML, PDF or JSON.
- **All cards:** Settings → Data → Export.
  - Private and encrypted cards are not exported.
  - Markdown exports put images in a `blobs` folder.
- **Backup:** the settings menu → *Back up now* writes a copy of the database to the `backups` folder. A backup is also made automatically before every database upgrade.

## Keyboard shortcuts and commands

- `Ctrl+Shift+P` opens the **command palette**, which lists every command.
- To change any shortcut, open Settings → General → *Keyboard shortcuts*. This opens `keymap.json`, which lists every action name. Restart Cardfile to apply your changes.

## Where are my files?

Open the settings menu and choose **Open data folder**. Unless you use one of the options below, it is:

- Windows: `%APPDATA%\FittyAr\Cardfile\data`
- Linux: `~/.local/share/FittyAr/Cardfile` (`$XDG_DATA_HOME/FittyAr/Cardfile` when set)
- macOS: `~/Library/Application Support/FittyAr/Cardfile`

Earlier versions used `~/.local/share/cardfile` on Linux and `~/Library/Application Support/ar.FittyAr.Cardfile` on macOS; Cardfile moves that folder to the new place the first time it starts. If it cannot be moved, Cardfile keeps using it.

The folder contains:

- `cardfile.db`: all your cards
- `backups/`: database backups
- `logs/`: logs, which help with bug reports
- `themes/`: extra themes, as gpui-component theme JSON files
- `keymap.json`: your shortcuts

Command line options:

- `cardfile --portable` stores data next to the executable.
- `cardfile --data-dir <folder>` uses any folder you choose.
