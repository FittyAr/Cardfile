# Cardfile

Cardfile é o meu gerenciador de fichas multiplataforma inspirado no estilo clássico do Windows 3.1, construído com Flet 1.0 Beta. Eu combino fluxo CRUD rápido, edição Markdown, autenticação e suporte multilíngue.

## Destaques

- Autenticação com hash de senha
- Ciclo completo de fichas (criar, editar, excluir, restaurar)
- Editor Markdown com pré‑visualização em tempo real
- Lixeira para recuperação
- Interface multilíngue
- Suporte desktop e web

## Requisitos

- Python 3.10+
- Docker (opcional)

## Começando

### Execução local

1. Crie o ambiente virtual:
```bash
python -m venv venv
```

2. Ative:
   - Windows (CMD): `.\venv\Scripts\Activate.bat`
   - Windows (PowerShell): `.\venv\Scripts\Activate.ps1`
   - Linux/Mac: `source venv/bin/activate`

3. Instale dependências:
```bash
pip install --upgrade pip
pip install -r requirements.txt
```

4. Execute:
```bash
python main.py
```

A app abre em `http://localhost:8550`.

### Docker

```bash
cd docker
docker-compose up --build
```

## Configuração

Você pode editar `config.json` para banco de dados, idioma e modo de execução.

## Documentação

- DeepWiki: https://deepwiki.com/FittyAr/Cardfile
- Eu mantenho documentação adicional em `docs/`.

## Contribuir

Contribuições são bem‑vindas:

- Abra issues com passos claros
- Envie PRs com mudanças focadas
- Melhore traduções

## Licença

Veja [LICENSE](../LICENSE) para detalhes.
