import os
import sys
import flet as ft

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "src"))

from cardfile.main import main

if __name__ == "__main__":
    ft.run(main)
