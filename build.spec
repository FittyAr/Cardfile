# -*- mode: python ; coding: utf-8 -*-
from PyInstaller.utils.hooks import collect_data_files

block_cipher = None

a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=[],
    datas=[
        ('config/*.json', 'config'),
        ('View/*.py', 'View'),
        ('Model/*.py', 'Model'),
        ('assets/*', 'assets'),
        ('database/*.db', 'database'),  # Agregado para la base de datos SQLite
    ],
    hiddenimports=[
        'flet',
        'flet_core',
        'sqlalchemy',
        'sqlalchemy.sql.default_comparator',
        'sqlalchemy.ext.declarative',
        'sqlalchemy.orm',
        'sqlite3',
        'bcrypt',
        'dotenv',
        'watchdog',
        'watchdog.observers',
        'watchdog.events',
        'websockets',
        'websockets.legacy',
        'websockets.legacy.client',
        'httpx',
        'anyio',
        'starlette',
        'starlette.routing',
        'uvicorn',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(
    a.pure,
    a.zipped_data,
    cipher=block_cipher
)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='Cardfile',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,  # Cambiado a True temporalmente para ver errores durante pruebas
    disable_windowed_traceback=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon='assets/icon.ico'
)

# Agregar información del programa
import plistlib
app = BUNDLE(
    exe,
    name='Cardfile.app',
    icon='assets/icon.ico',
    bundle_identifier=None,
    version='1.0.0',
    info_plist={
        'NSHighResolutionCapable': 'True',
        'CFBundleShortVersionString': '1.0.0',
        'CFBundleVersion': '1.0.0'
    }
) 