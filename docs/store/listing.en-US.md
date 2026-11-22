# Microsoft Store listing — English (United States)

Texts for each field of the Partner Center listing. Paste them as they are.

## Product name

NoteChest

## Description

NoteChest is a private, fast notes app that works like a modern card file. Write on cards, organize them with tags, find anything in an instant and stay in control of your data: everything is stored on your computer, with no account and no telemetry.

WRITE LIKE IN A WORD PROCESSOR
• Headings, lists, checklists, tables, code blocks, colors, images and attachments. There is no Markdown to learn.
• Export a card, or all of them, to Markdown, HTML, PDF or JSON.

FIND AND ORGANIZE
• Instant full-text search across titles and contents.
• Tags, favorites, pinned cards, manual ordering, a recycle bin and version history.

PROTECT WHAT MATTERS
• Protect each card: read-only, private (a password to view it) or encrypted at rest.
• Choose a single password, several users or no password at all, and share cards with other users of the same computer.
• Automatic daily backups, and restore from any backup.

YOURS, ON YOUR TERMS
• Works offline. Your cards never leave your PC unless you export them.
• Portable mode, to carry your cards on a USB drive.
• 21 interface languages, including right-to-left scripts, and spell checking.
• Optional AI assistant: connect your own cloud or local AI service (OpenAI, Claude, Gemini, Ollama, LM Studio and more) to search, rewrite, format and tag your cards by chat. It stays hidden until you set it up, asks before it changes or deletes anything, and every change it makes is attributed to the AI and can be undone. Its answers are generated live by the service you choose and can be wrong or inappropriate: use the Report button in the assistant panel to flag them.
• A local MCP server (off by default) so AI apps such as Claude or Cursor can use your cards.

NoteChest is free and open source (MIT or Apache-2.0). It was previously called Cardfile: your existing cards are picked up automatically.

## What's new in this version

First stable release. Cardfile is now NoteChest, with a new identity and a Microsoft Store package. Your existing data is adopted automatically: cards, backups, settings and encrypted cards keep working. New: the setup starts in your system language, the window remembers its size and position, you can move your data folder from Settings, and daily backups are on by default.

## Product features

- Word-like editor: headings, lists, checklists, tables, code blocks, images and attachments
- Instant full-text search, tags, favorites, pinned cards and version history
- Read-only, private and encrypted cards, with a password or several users
- Automatic daily backups, restore from any backup and a portable mode for USB drives
- Export to Markdown, HTML, PDF and JSON
- 21 languages, right-to-left scripts and spell checking
- Optional AI assistant with your own cloud or local AI; every AI change can be undone
- Free, open source, works offline, no account and no telemetry

## Search terms (7 at most)

notes, cards, card file, cardfile, encrypted notes, markdown, offline notes

## Copyright and trademark info

© 2026 FittyAr. Open source under the MIT or Apache-2.0 license.

## Additional license terms

NoteChest is free software, available under the MIT license or the Apache License 2.0, at your option. Source code and licenses: https://github.com/FittyAr/NoteChest

## Screenshots (Desktop), in this order

Files in `screenshots/en-US/`, 1600 x 900 PNG. Captions (200 characters at most):

1. `01-editor-dark.png` — A card with a table, a checklist and a quote, in the dark theme.
2. `02-search.png` — Instant full-text search highlights the matches in every card.
3. `03-light-theme.png` — The same card in the light theme. There are 14 themes to choose from.
4. `04-private-cards.png` — Private and encrypted cards stay locked until you enter your password.
5. `05-many-languages.png` — Chinese, Japanese, Korean, Hindi, Hebrew and Arabic, with right-to-left support.
6. `06-code-blocks.png` — Code blocks and inline code for technical notes.
7. `07-settings.png` — Settings: theme, language, editor size and optional AI services.

## Store logos

- 1:1 app tile icon, 300 x 300: `logos/app-tile-icon-300.png`
- 1:1 box art, 1080 x 1080: `logos/box-art-1080x1080.png` (a 2160 x 2160 version is also there)
- 2:3 poster art, 720 x 1080: `logos/poster-art-720x1080.png` (a 1440 x 2160 version is also there)
- 16:9 super hero art (Trailers and additional assets), 1920 x 1080, no text: `logos/hero-art-1920x1080.png`

## Product properties and submission options

- Category: Productivity.
- Support contact info (website): https://github.com/FittyAr/NoteChest/issues
- Website: https://github.com/FittyAr/NoteChest
- Privacy policy URL: https://github.com/FittyAr/NoteChest/blob/main/docs/PRIVACY.md
- Price: free. Markets: all. Visibility: public.
- Uses live generative AI: yes (see the Notes for certification).

## Notes for certification

NoteChest is a local notes app for Windows. It needs no account or sign-in: a first-run wizard sets it up, and then it works offline.

Restricted capabilities. runFullTrust: NoteChest is a Win32 desktop application written in Rust. unvirtualizedResources (AppData write virtualization is disabled): NoteChest keeps the user's notes in a SQLite database under %APPDATA%\FittyAr\NoteChest. With AppData write virtualization, the database journal (WAL), the lock file and the backups would be redirected to a per-package folder that Windows deletes when the app is uninstalled, which would lose notes, and a copy installed outside the Store would open the same database with a different journal and lock, which can corrupt it. Disabling write virtualization keeps the notes in one real location that survives uninstalling or reinstalling the app.

Live generative AI (policy 11.16). The assistant is optional and hidden until the user connects their own AI service with their own API key. It is disclosed in the description. It asks for confirmation before changing or deleting cards, never sends encrypted cards, and users report inappropriate answers with the Report button in the assistant panel (it opens a blank issue form: nothing is sent automatically). To try it you need your own API key; the rest of the app needs no network.

The update check is disabled in the Store package: the Store updates the app. Privacy policy: https://github.com/FittyAr/NoteChest/blob/main/docs/PRIVACY.md
