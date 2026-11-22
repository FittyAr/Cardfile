# NoteChest user guide

Español: [GUIA_DE_USUARIO.md](GUIA_DE_USUARIO.md)

## First run

The first time you start NoteChest, a short wizard opens in your system's language (you can
pick another one on its first screen; it becomes your account's language) and asks four things:

1. **Storage.**
   - *Standard* keeps your cards in your user profile.
   - *Portable* keeps them in a `data` folder next to NoteChest, which is handy on a USB drive.
2. **Sign-in.**
   - *No password*: NoteChest opens directly.
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
- **Audio plays inside the card.** MP3, WAV, OGG and FLAC audio get a player with play/pause, a seek bar and mute. Video, AAC/M4A and other formats open in your system player; if no application on your computer can open them, NoteChest tells you.

### Other languages and scripts

Chinese, Japanese and Korean text works everywhere, including PDF export (NoteChest uses a CJK font installed on your system). Right-to-left text (Hebrew, Arabic, Persian) is shown in the right order and right-to-left paragraphs are aligned to the right. Known limit: one-line text boxes (such as the card title while you edit it) still show right-to-left text left to right.

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

With a password or several users, NoteChest can also **lock itself** after a period of inactivity. Lock it now with `Ctrl+Shift+L`.

## Sharing cards

With several users (multi-user mode), open a card's **⋯** menu → *Share…* (or the people button) and choose, for each user, **No access**, **Can view** or **Can edit**. Shared cards appear under **Shared with me** in the sidebar. Only the owner can delete, protect, tag or share a card. People who can edit see who made the last change in the status bar and in the version history.

To share an **encrypted** card, unlock it first. The other person opens it with *their own* lock password; they need to have signed in (or unlocked a protected card) once so that NoteChest created their keys.

## AI assistant

NoteChest can work with an AI, in the cloud or on your computer. Nothing AI-related appears until you set it up.

1. Open **Settings → AI → Add AI service** and pick a preset: OpenAI, Anthropic (Claude), Google Gemini, Mistral, Groq, DeepSeek, xAI, OpenRouter, Together AI, Perplexity, Azure OpenAI, **Ollama** or **LM Studio** (local), or *Custom* for any compatible service.
2. Enter your API key (local services don't need one), choose a model and press **Test connection**.
3. The **assistant button** (robot icon above the card list, or `Ctrl+Shift+A`) opens the chat panel.

Ask in your own words: find cards, rewrite or format a card, turn notes into a checklist, add tags, mark favorites, reorder cards, move cards to the recycle bin… Every change is recorded as made by **NoteChest AI** on your behalf, the previous version is kept in the history, and deleted cards go to the recycle bin, so you can always undo. The assistant never sees encrypted cards; private cards only if you allow it in Settings → AI.

## Using NoteChest from other AI apps (MCP)

NoteChest includes an MCP server so AI apps such as Claude Desktop, Claude Code, Cursor or VS Code can use your cards while NoteChest is open.

1. In **Settings → AI**, turn on **Run the MCP server** (it listens on `127.0.0.1` only; default port 7337).
2. Under **API keys**, create a key. Choose *Read only* or *Allow private cards* if you want. Copy the key: it is shown only once, together with ready-to-paste settings for each AI app.
3. The AI app sees exactly what you see. Its changes are recorded as made by NoteChest AI, through that key. **Revoke** a key to cut its access immediately. The list shows when each key was last used, and *Recent AI activity* shows what the AI changed.

## Updates

NoteChest checks for new versions at startup (Settings → General). When one is available, click the notice, read what's new and choose **Update and restart**: NoteChest downloads it, checks its signature and installs it. Copies installed from the Microsoft Store or a Linux package manager are updated by them instead.

## Export and backup

- **One card:** the **⋯** menu → Markdown, HTML, PDF or JSON.
- **All cards:** Settings → Data → Export.
  - Private and encrypted cards are not exported.
  - Markdown exports put images in a `blobs` folder.
- **Backup:** the settings menu → *Back up now* writes a copy of the database to the `backups` folder. A backup is also made automatically before every database upgrade.

## Keyboard shortcuts and commands

- `Ctrl+Shift+P` opens the **command palette**, which lists every command.
- To change any shortcut, open Settings → General → *Keyboard shortcuts*. This opens `keymap.json`, which lists every action name. Restart NoteChest to apply your changes.

## Where are my files?

Open the settings menu and choose **Open data folder**. Unless you use one of the options below, it is:

- Windows: `%APPDATA%\FittyAr\NoteChest\data`
- Linux: `~/.local/share/FittyAr/NoteChest` (`$XDG_DATA_HOME/FittyAr/NoteChest` when set)
- macOS: `~/Library/Application Support/FittyAr/NoteChest`

Earlier versions used `~/.local/share/notechest` on Linux and `~/Library/Application Support/ar.FittyAr.NoteChest` on macOS; NoteChest moves that folder to the new place the first time it starts. If it cannot be moved, NoteChest keeps using it.

The folder contains:

- `notechest.db`: all your cards
- `backups/`: database backups
- `logs/`: logs, which help with bug reports
- `themes/`: extra themes, as gpui-component theme JSON files
- `keymap.json`: your shortcuts

Command line options:

- `notechest --portable` stores data next to the executable.
- `notechest --data-dir <folder>` uses any folder you choose.


## Portable folders and recovery

The Windows setup offers **Install on this computer**, **Create a portable copy** and
an explicit all-users installation. For portable use, choose a writable folder on a USB
 drive, OneDrive, or another local disk. Open `notechest.exe` in that folder. Move the
entire folder, including `portable.flag` and `data/`, together. ZIP downloads remain available.

PowerShell uses the same verified setup executable:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/FittyAr/NoteChest/main/install.ps1))) -Portable -Destination "E:\NoteChest"
```

To include an existing collection, an administrator can use **Settings → Data → Create
portable copy**. Select the parent folder; NoteChest creates a new `NoteChest portable`
subfolder and refuses to overwrite an existing destination. All accounts, cards,
attachments, history and card encryption are retained. MCP and automatic backups are
disabled in the copy. Re-enter AI keys stored in the original computer's credential store.
Store and system-package distributions instead direct you to the portable download.

Administrators can choose a backup folder in **Settings → Data**, create a backup now,
and turn daily backups on or off. New installations make a daily copy in `backups/` by
default and keep the 10 newest automatic copies (portable copies start with it off). Checks run hourly while signed
in; a missed daily copy runs after sign-in. An unavailable folder reports an error and
is retried. Manual and safety copies are never removed by automatic retention.

**Restore backup** replaces the entire collection, including users and permissions;
it does not merge cards. NoteChest validates the selected `.db`, saves current edits,
creates a local safety backup, prepares the replacement and restarts. Use the restored
accounts/passwords. Local backup preferences and window dimensions stay unchanged.
Backups preserve existing encryption but do not encrypt otherwise unencrypted cards.
Passwords and external credential-store secrets cannot be recovered from a backup.

Close NoteChest before removing a USB drive or changing computers. With OneDrive, wait
until synchronization finishes before opening the folder on another computer; simultaneous
use is unsupported. NoteChest shows a warning at sign-in when its data is in a folder synced
by OneDrive, Dropbox, Google Drive or a similar service. The local process lock does not coordinate different computers.
The app remembers the window's size, position and maximized state in `local-config.json`
and reopens on the same monitor while it is connected.

**Move the data folder.** Administrators can move the whole collection with **Settings →
Data → Move…**. NoteChest copies the database (a consistent snapshot), backups, themes,
shortcuts, dictionaries and local preferences to a new `NoteChest` folder inside the one
you choose, then restarts using it. The previous folder is left untouched; delete it
once you have checked the new one. The new location is remembered in
`data-location.txt` in the user configuration folder (`%APPDATA%\FittyAr\NoteChest\config`
on Windows, `~/.config/notechest` on Linux). If that folder later goes missing
(for example, an unplugged drive), NoteChest says so at start instead of creating an empty
collection; delete `data-location.txt` to go back to the default folder. Portable copies
and folders set with `--data-dir` cannot be moved this way.


## Update channel

Settings → General → Update channel has a dropdown with **Recommended (Latest)**
(default) and **Preview (Prerelease)**. Latest follows the release designated Latest
on GitHub, even if its tag contains `beta`. Prerelease also includes testing releases
that may still have bugs. Both channels verify signed
checksums and asset signatures; neither downgrades the installed version. Changing
the choice checks immediately, and it is remembered for your account.
