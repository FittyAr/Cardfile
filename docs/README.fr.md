# Cardfile

Cardfile est un gestionnaire de fiches multiplateforme inspiré du style classique de Windows 3.1, construit avec Flet 1.0 Beta. Il combine un flux CRUD rapide, l’édition Markdown, l’authentification et le support multilingue.

## Points forts

- Authentification avec hachage de mot de passe
- Cycle complet des fiches (créer, modifier, supprimer, restaurer)
- Éditeur Markdown avec aperçu en temps réel
- Corbeille pour la récupération
- Interface multilingue
- Support desktop et web

## Prérequis

- Python 3.10+
- Docker (optionnel)

## Démarrage rapide

### Exécution locale

1. Créez l’environnement virtuel :
```bash
python -m venv venv
```

2. Activez‑le :
   - Windows (CMD): `.\venv\Scripts\Activate.bat`
   - Windows (PowerShell): `.\venv\Scripts\Activate.ps1`
   - Linux/Mac: `source venv/bin/activate`

3. Installez les dépendances :
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. Lancez l’application :
```bash
python main.py
```

L’app s’ouvre sur `http://localhost:8550`.

### Docker

```bash
cd docker
docker-compose up --build
```

## Configuration

Modifiez `config.json` pour la base de données, la langue et le mode d’exécution.

## Documentation

- DeepWiki : https://deepwiki.com/FittyAr/Cardfile
- Documentation complémentaire dans `docs/`.

## Contribuer

Les contributions sont les bienvenues :

- Ouvrez des issues avec des étapes claires
- Envoyez des PRs avec des changements ciblés
- Améliorez les traductions

## Licence

Voir [LICENSE](../LICENSE) pour les détails.
