# Mejoras al Editor Markdown

## Problema Resuelto

El editor markdown tenía los siguientes problemas:

1. **No detectaba la selección del usuario**: Cuando el usuario seleccionaba una o dos palabras y hacía clic en un botón de formato (como negrita), el formato se aplicaba al final del texto en lugar de a la selección.

2. **No respetaba la posición del cursor**: Si el usuario no seleccionaba texto y solo dejaba el cursor en un punto, el formato `**texto**` aparecía al final del texto en lugar de donde estaba el cursor.

## Solución Implementada

### Archivos Modificados

1. **`View/MarkdownEditor.py`** - Versión principal del editor
2. **`View/components/markdown_editor.py`** - Componente reutilizable

### Cambios Realizados

#### 1. Uso del Evento `on_selection_change`

Flet proporciona el evento `on_selection_change` que se dispara cada vez que el usuario cambia la selección o mueve el cursor. Este evento incluye:

- **`e.selection.start`**: Posición de inicio de la selección
- **`e.selection.end`**: Posición de fin de la selección
- **`e.selected_text`**: El texto seleccionado

La solución ahora usa este evento para rastrear la selección del usuario en tiempo real:

```python
# Variables para rastrear la selección actual
current_selection_start: int = 0
current_selection_end: int = 0

def _on_selection_change(e: ft.TextSelectionChangeEvent):
    """Callback que rastrea los cambios de selección"""
    nonlocal current_selection_start, current_selection_end
    if e.selection:
        current_selection_start = e.selection.start if e.selection.start is not None else 0
        current_selection_end = e.selection.end if e.selection.end is not None else 0

# Conectar el evento de selección
target_field.on_selection_change = _on_selection_change
```

#### 2. Lógica de Formato Mejorada

La función `_wrap()` ahora usa las variables rastreadas para aplicar el formato:

**Caso 1: Hay selección (start != end)**
- Envuelve el texto seleccionado con el formato
- Soporta toggle: si el texto ya está formateado, lo desformatea

**Caso 2: No hay selección (start == end)**
- Inserta el placeholder `**texto**` en la posición del cursor
- NO al final del texto
- NO intenta expandir a la palabra bajo el cursor

```python
def _wrap(prefix: str, suffix: Optional[str] = None) -> None:
    # Obtener selección rastreada
    start = max(0, min(current_selection_start, len(value)))
    end = max(0, min(current_selection_end, len(value)))
    
    if start != end:
        # HAY SELECCIÓN: envolver el texto seleccionado
        selected = value[start:end]
        new_value = value[:start] + f"{prefix}{selected}{suffix}" + value[end:]
    else:
        # SIN SELECCIÓN: insertar en la posición del cursor
        placeholder = "texto"
        new_value = value[:start] + f"{prefix}{placeholder}{suffix}" + value[start:]
```

## Comportamiento Esperado

### Escenario 1: Usuario selecciona texto
```
Texto original: "Este es un texto de prueba"
Usuario selecciona: "texto"
Hace clic en negrita
Resultado: "Este es un **texto** de prueba"
```

### Escenario 2: Usuario posiciona el cursor sin seleccionar
```
Texto original: "Este es un | de prueba"  (| = cursor)
Hace clic en negrita
Resultado: "Este es un **texto**| de prueba"
```

### Escenario 3: Texto vacío
```
Texto original: ""
Hace clic en negrita
Resultado: "**texto**"
```

## Ventajas de la Solución

**✅ Usa la API oficial de Flet**: El evento `on_selection_change` es la forma documentada y soportada de rastrear la selección del usuario.

**✅ Funciona en todas las plataformas**: Al usar la API oficial, la solución funciona consistentemente en web, desktop y mobile.

**✅ Rastreo en tiempo real**: La selección se actualiza automáticamente cada vez que el usuario cambia la selección o mueve el cursor.

**✅ Código más limpio**: No necesitamos intentar múltiples propiedades no documentadas ni usar fallbacks complejos.

## Pruebas Recomendadas

1. **Seleccionar una palabra y aplicar negrita**
2. **Seleccionar varias palabras y aplicar cursiva**
3. **Posicionar el cursor en medio del texto y aplicar código**
4. **Aplicar formato en texto vacío**
5. **Aplicar formato al final del texto**
6. **Probar el toggle**: Seleccionar `**texto**` y aplicar negrita para quitarlo

## Referencias

- [TextField - Flet Documentation](https://docs.flet.dev/controls/textfield/)
- [TextSelectionChangeEvent - Flet Documentation](https://docs.flet.dev/types/textselectionchangeevent/)
- [Handling selection changes - Example](https://docs.flet.dev/controls/textfield/#handling-selection-changes)
