# Cardfile

Cardfile is a cross‑platform card/file manager inspired by the classic Windows 3.1 aesthetic, built with Flet 1.0 Beta. It combines a fast CRUD workflow with Markdown editing, authentication, and multi‑language support.

## Highlights

- Authentication with password hashing
- Full card lifecycle (create, edit, delete, restore)
- Markdown editor with live preview
- Recycle bin for recovery
- Multi‑language UI (Spanish, English, Portuguese BR, French, German, Russian, Chinese)
- Desktop and web runtime support

## Requirements

- Python 3.10+
- Docker (optional, for containerized runs)

## Getting Started

### Local Run

1. Create a virtual environment:
```bash
python -m venv venv
```

2. Activate it:
   - Windows (CMD): `.\venv\Scripts\Activate.bat`
   - Windows (PowerShell): `.\venv\Scripts\Activate.ps1`
   - Linux/Mac: `source venv/bin/activate`

3. Install dependencies:
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. Run the app:
```bash
python main.py
```

The app opens at `http://localhost:8550`.

### Docker Run

1. Build and run:
```bash
cd docker
docker-compose up --build
```

2. Open:
`http://localhost:8550`

Stop the container:
```bash
docker-compose down
```

## Configuration

Edit `config.json` to customize database settings, default language, and runtime mode.

## Project Structure

```
Cardfile-py/
├── src/cardfile/     # Application package
├── lang/             # Translations
├── assets/           # Icons and assets
├── docker/           # Docker setup
├── main.py           # Entry point
└── requirements.txt  # Python dependencies
```

## Documentation

- DeepWiki index: https://deepwiki.com/FittyAr/Cardfile
- Additional docs are in `docs/`, including build and release notes.

## Readmes in Other Languages

- Español: `docs/README.es.md`
- Português (BR): `docs/README.pt-BR.md`
- Français: `docs/README.fr.md`

## Contributing

Contributions are welcome. If you want to help:

- Open issues with clear repro steps or feature proposals
- Send PRs with focused, well‑scoped changes
- Add translations or improve existing ones

## License

See [LICENSE](LICENSE) for details.
