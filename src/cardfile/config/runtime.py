import flet as ft
import os
import platform


def is_web_runtime(page: ft.Page) -> bool:
    """Detect if the app is running in a web browser."""
    client_platform = getattr(page, "platform", None)
    if client_platform is not None:
        platform_name = str(client_platform).lower()
        if "web" in platform_name:
            return True
    return bool(getattr(page, "web", False))


def get_os_platform() -> str:
    """
    Detect the underlying OS platform.
    Returns: 'windows', 'linux', or 'docker'
    """
    if os.path.exists('/.dockerenv'):
        return 'docker'
    
    system = platform.system().lower()
    if 'windows' in system:
        return 'windows'
    if 'linux' in system:
        return 'linux'
    
    return system


def get_data_dir(app_name: str, portable: bool = False) -> str:
    """
    Get the recommended data directory for the current platform.
    """
    if portable:
        # Portable mode: same directory as the script/executable
        import sys
        if getattr(sys, "frozen", False):
            return os.path.dirname(sys.executable)
        # Returns src/cardfile as the data directory for portable mode
        return os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

    os_type = get_os_platform()
    
    if os_type == 'windows':
        return os.path.join(os.environ.get('APPDATA', os.path.expanduser('~')), app_name)
    elif os_type == 'docker':
        # In Docker, we typically mount /app or /app/data for persistence
        if os.path.exists('/app/data'):
            return '/app/data'
        return '/app'
    elif os_type == 'linux':
        # Follow XDG Base Directory Specification or fallback to ~/.local/share
        base = os.environ.get('XDG_DATA_HOME', os.path.expanduser('~/.local/share'))
        return os.path.join(base, app_name)
    
    return os.path.expanduser(f'~/.{app_name.lower()}')
