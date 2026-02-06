# Correcciones al Editor Markdown - Segunda Iteración

## Problemas Reportados y Soluciones

### 1. ❌ Cursiva no funcionaba (mostraba asteriscos)

**Problema**: El botón de cursiva usaba `*` pero Markdown estándar requiere `_` para cursiva o `**` para negrita.

**Solución**: Cambiar de `*` a `_` para cursiva:

```python
# Antes
on_click=lambda e: _wrap("*")

# Después  
on_click=lambda e: _wrap("_")
```

### 2. ❌ Listas, citas, enlaces y tareas agregaban al final

**Problema**: Estos botones usaban `_append()` que siempre agrega al final del texto, ignorando la selección del usuario.

**Solución**: 
- **Listas y citas**: Usar `_block()` para aplicar formato de bloque a las líneas seleccionadas
- **Enlaces e imágenes**: Usar `_wrap()` para envolver la selección
- **Checklist**: Usar `_block()` para agregar el prefijo a las líneas

```python
# Enlaces - Antes
on_click=lambda e: _append("[texto](https://)")

# Enlaces - Después
on_click=lambda e: _wrap("[", "](https://)")

# Checklist - Antes
on_click=lambda e: _append("\n- [ ] tarea\n")

# Checklist - Después
on_click=lambda e: _block("- [ ] ")
```

### 3. ⚠️ Código inline se ve mal en el visor

**Problema**: El control `ft.Markdown` de Flet tiene limitaciones en el renderizado de código inline. Esto es una **limitación del framework Flet**, no de nuestro código.

**Workarounds posibles**:

1. **Usar bloque de código** en lugar de inline para mejor visualización
2. **Configurar el tema de código** en el Markdown viewer:
   ```python
   ft.Markdown(
       value=content,
       code_theme=ft.MarkdownCodeTheme.GITHUB,  # o ATOM_ONE_DARK
       extension_set=ft.MarkdownExtensionSet.GITHUB_WEB,
   )
   ```

3. **Esperar actualizaciones de Flet** que mejoren el renderizado

### 4. ✅ Bloque de código mejorado

**Cambio**: Ahora el bloque de código usa `_block()` en lugar de `_wrap()` para mejor manejo de múltiples líneas:

```python
# Antes
on_click=lambda e: _wrap("```\n", "\n```")

# Después
on_click=lambda e: _block("```\n")
```

## Resumen de Cambios

### Archivo: `View/MarkdownEditor.py`

| Botón | Antes | Después | Función |
|-------|-------|---------|---------|
| Cursiva | `_wrap("*")` | `_wrap("_")` | `_wrap()` |
| Bloque código | `_wrap("```\n", "\n```")` | `_block("```\n")` | `_block()` |
| Enlace | `_append("[texto](...)")` | `_wrap("[", "](...)")` | `_wrap()` |
| Imagen | `_append("![alt](...)")` | `_wrap("![", "](...)")` | `_wrap()` |
| Checklist | `_append("\n- [ ] tarea\n")` | `_block("- [ ] ")` | `_block()` |

### Archivo: `View/components/markdown_editor.py`

Los mismos cambios aplicados para mantener consistencia.

## Comportamiento Esperado Ahora

### Cursiva
```
Seleccionas: "texto"
Resultado: "_texto_"
Renderizado: texto (en cursiva)
```

### Listas
```
Seleccionas dos párrafos:
Párrafo uno
Párrafo dos

Resultado:
- Párrafo uno
- Párrafo dos
```

### Enlaces
```
Seleccionas: "Google"
Resultado: "[Google](https://)"
```

### Checklist
```
Seleccionas dos líneas:
Tarea uno
Tarea dos

Resultado:
- [ ] Tarea uno
- [ ] Tarea dos
```

## Limitaciones Conocidas de Flet Markdown

### 1. Renderizado de Código Inline
El código inline (`` `código` ``) puede verse mal dependiendo del tema y configuración. Esto es una limitación del widget `ft.Markdown`.

**Recomendación**: Usar bloques de código para mejor visualización.

### 2. Temas de Código Limitados
Flet solo soporta algunos temas predefinidos:
- `GITHUB`
- `ATOM_ONE_DARK`
- `ATOM_ONE_LIGHT`
- etc.

### 3. Extensiones de Markdown
Algunas extensiones avanzadas de Markdown pueden no funcionar correctamente. Usar `extension_set=ft.MarkdownExtensionSet.GITHUB_WEB` para mejor compatibilidad.

## Pruebas Recomendadas

1. ✅ **Cursiva**: Seleccionar texto y aplicar cursiva → debe verse en cursiva
2. ✅ **Listas**: Seleccionar múltiples párrafos y aplicar lista → cada párrafo debe tener `-`
3. ✅ **Enlaces**: Seleccionar texto y aplicar enlace → debe envolver con `[texto](https://)`
4. ✅ **Checklist**: Seleccionar líneas y aplicar checklist → cada línea debe tener `- [ ]`
5. ⚠️ **Código inline**: Puede verse mal en el visor (limitación de Flet)

## Referencias

- [Flet Markdown Control](https://docs.flet.dev/controls/markdown/)
- [Markdown Extension Set](https://docs.flet.dev/types/markdownextensionset/)
- [Markdown Code Theme](https://docs.flet.dev/types/markdowncodetheme/)
