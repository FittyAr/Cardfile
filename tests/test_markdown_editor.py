"""
Script de prueba para el editor Markdown con detección de selección.

Este script demuestra cómo funciona el nuevo sistema de detección de selección
usando el evento on_selection_change de Flet.

Para ejecutar:
    python test_markdown_editor.py
"""
import flet as ft
from View.components.markdown_editor import create_markdown_toolbar


def main(page: ft.Page):
    page.title = "Prueba de Editor Markdown"
    page.padding = 20
    
    # Texto de ejemplo
    sample_text = """# Editor Markdown

Este es un texto de **ejemplo** para probar el editor.

Puedes:
- Seleccionar texto y aplicar formato
- Hacer clic en cualquier posición y el formato se insertará ahí
- Probar los diferentes botones de la barra de herramientas

## Instrucciones de Prueba

1. **Selecciona** una palabra (por ejemplo, "ejemplo") y haz clic en el botón de negrita
2. **Posiciona el cursor** en medio de una línea (sin seleccionar) y haz clic en cursiva
3. **Selecciona varias palabras** y aplica código
4. **Prueba el toggle**: Selecciona texto ya formateado y vuelve a aplicar el mismo formato

¡Diviértete probando!
"""
    
    # Estado para mostrar información de selección
    selection_info = ft.Text("Selección: Ninguna", size=12, color=ft.Colors.BLUE_GREY_400)
    
    def update_selection_info(e: ft.TextSelectionChangeEvent):
        """Actualiza la información de selección en la UI"""
        if e.selection:
            start = e.selection.start if e.selection.start is not None else 0
            end = e.selection.end if e.selection.end is not None else 0
            
            if start == end:
                selection_info.value = f"Cursor en posición: {start}"
            else:
                selected = e.selected_text if e.selected_text else ""
                selection_info.value = f"Selección: [{start}:{end}] = '{selected[:30]}{'...' if len(selected) > 30 else ''}'"
        else:
            selection_info.value = "Selección: Ninguna"
        selection_info.update()
    
    # Crear el editor
    editor = ft.TextField(
        value=sample_text,
        multiline=True,
        min_lines=15,
        max_lines=20,
        expand=True,
        border_color=ft.Colors.BLUE_GREY_700,
        focused_border_color=ft.Colors.BLUE_400,
    )
    
    # Agregar listener adicional para mostrar info
    original_on_selection_change = None
    
    def combined_selection_change(e):
        update_selection_info(e)
        if original_on_selection_change:
            original_on_selection_change(e)
    
    # Crear la barra de herramientas
    toolbar = create_markdown_toolbar(editor)
    
    # Guardar el handler original y reemplazarlo con el combinado
    original_on_selection_change = editor.on_selection_change
    editor.on_selection_change = combined_selection_change
    
    # Construir la UI
    page.add(
        ft.Column(
            [
                ft.Text("🎨 Prueba del Editor Markdown", size=24, weight=ft.FontWeight.BOLD),
                ft.Divider(),
                selection_info,
                ft.Container(height=10),
                toolbar,
                ft.Container(height=10),
                ft.Container(
                    content=editor,
                    border=ft.border.all(1, ft.Colors.BLUE_GREY_700),
                    border_radius=8,
                    padding=10,
                    expand=True,
                ),
                ft.Container(height=10),
                ft.Text(
                    "💡 Tip: Selecciona texto o posiciona el cursor y usa los botones de la barra de herramientas",
                    size=12,
                    italic=True,
                    color=ft.Colors.BLUE_GREY_400,
                ),
            ],
            expand=True,
        )
    )


if __name__ == "__main__":
    ft.app(target=main)
