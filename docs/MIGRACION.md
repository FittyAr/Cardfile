# Guía de Migración a Flet 1.0 Beta

## Resumen de Cambios Realizados

Este documento detalla todos los cambios realizados para migrar Cardfile-py de Flet 0.28.3 a Flet 1.0 Beta (0.80.0).

## Cambios Principales

### 1. Punto de Entrada (`main.py`)

- **Antes**: `flet.app(target=main)`
- **Después**: `ft.app(target=main, view=AppView.WEB_BROWSER)`
- **Nota**: Se agregó el import de `AppView` y se cambió el método de ejecución para usar modo web con AppView.WEB_BROWSER

### 2. Almacenamiento de Cliente

- **Antes**: `page.client_storage.get/set()`
- **Después**: `page.shared_preferences.get/set()`
- **Archivos afectados**: 
  - `main.py`
  - `View/Login.py`
  - `View/Card.py`
  - `View/NewCard.py`
  - `View/EditCard.py`
  - `View/Recycle.py`
  - `config/Route.py`

### 3. Propiedades de Colores e Iconos

- **Antes**: `ft.Colors.BLUE`, `ft.Icons.ADD`
- **Después**: `ft.colors.BLUE`, `ft.icons.ADD`
- **Archivos afectados**: Todos los archivos en `View/`

### 4. Botones y Controles de Texto

- **Antes**: `ft.ElevatedButton(text="...")`, `ft.TextButton(text="...")`
- **Después**: `ft.ElevatedButton(content=ft.Text("..."))`, `ft.TextButton(content=ft.Text("..."))`
- **Archivos afectados**:
  - `View/Login.py`
  - `View/NewCard.py`
  - `View/EditCard.py`
  - `View/Recycle.py`
  - `View/newUser.py`

### 5. Iconos

- **Antes**: `Icon.name = ft.Icons.XXX`
- **Después**: `Icon.icon = ft.icons.XXX`
- **Archivos afectados**:
  - `View/Card.py`
  - `View/MarkdownEditor.py`

### 6. Diálogos

- **Antes**: 
  ```python
  page.dialog = dlg_modal
  dlg_modal.open = True
  page.update()
  ```
- **Después**:
  ```python
  page.show_dialog(dlg_modal)
  # Para cerrar: page.pop_dialog()
  ```
- **Archivos afectados**:
  - `View/Card.py`
  - `View/Recycle.py`

### 7. Alineación

- **Antes**: `ft.alignment.center`
- **Después**: `ft.Alignment.CENTER`
- **Archivos afectados**: Todos los archivos con contenedores

### 8. Padding

- **Antes**: `padding=30` o `padding=ft.padding.symmetric(8, 8)`
- **Después**: `padding=ft.padding.all(30)` o `padding=ft.padding.symmetric(vertical=8, horizontal=8)`
- **Archivos afectados**: Todos los archivos con contenedores

### 9. Manejo de Eventos de Botones en Diálogos

- **Antes**: `e.control.text == "..."` 
- **Después**: 
  ```python
  button_text = e.control.content.value if hasattr(e.control.content, 'value') else str(e.control.content)
  if button_text == "..."
  ```
- **Archivos afectados**:
  - `View/Card.py`
  - `View/Recycle.py`

## Archivos Modificados

1. `main.py` - Migración principal
2. `requirements.txt` - Actualización de dependencias
3. `config/Route.py` - Actualización de rutas
4. `View/Login.py` - Migración completa
5. `View/Card.py` - Migración completa (archivo más complejo)
6. `View/NewCard.py` - Migración completa
7. `View/EditCard.py` - Migración completa
8. `View/Recycle.py` - Migración completa
9. `View/newUser.py` - Migración completa
10. `View/Navigation.py` - Migración completa
11. `View/MarkdownEditor.py` - Migración completa

## Archivos Nuevos

1. `ANALISIS.md` - Documento de análisis técnico completo
2. `MIGRACION.md` - Este documento
3. `docker/Dockerfile` - Configuración Docker
4. `docker/docker-compose.yml` - Orquestación Docker
5. `docker/.dockerignore` - Exclusiones Docker

## Dependencias Actualizadas

- **Flet**: 0.28.3 → 0.80.0 (Flet 1.0 Beta)
- Todas las demás dependencias actualizadas a sus últimas versiones compatibles

## Instrucciones para Probar

### Opción 1: Ejecución Local (requiere Python 3.10+)

```bash
# Crear entorno virtual
python -m venv venv

# Activar entorno virtual
# Windows:
venv\Scripts\activate
# Linux/Mac:
source venv/bin/activate

# Instalar dependencias
pip install -r requirements.txt

# Ejecutar aplicación
python main.py
```

### Opción 2: Ejecución con Docker

```bash
# Construir imagen
cd docker
docker-compose build

# Ejecutar contenedor
docker-compose up

# La aplicación estará disponible en http://localhost:8550
```

## Notas Importantes

1. **Modo Web**: La aplicación ahora se ejecuta en modo web usando `AppView.WEB_BROWSER` target de Flet 1.0
2. **Base de Datos**: La base de datos SQLite se creará automáticamente en la primera ejecución
3. **Puerto**: Por defecto, la aplicación usa el puerto 8550 para modo web
4. **Breaking Changes**: Esta migración incluye cambios incompatibles con Flet 0.28.3

## Problemas Conocidos y Soluciones

### Si la aplicación no inicia:

1. Verificar que Python 3.10+ está instalado
2. Verificar que todas las dependencias están instaladas: `pip install -r requirements.txt`
3. Verificar que no hay conflictos de puerto (8550)

### Si hay errores de importación:

1. Verificar que el entorno virtual está activado
2. Reinstalar dependencias: `pip install --upgrade -r requirements.txt`

### Si los diálogos no funcionan:

- Verificar que se usa `page.show_dialog()` en lugar de `page.dialog`
- Verificar que se usa `page.pop_dialog()` para cerrar

## Próximos Pasos Recomendados

1. Probar todas las funcionalidades después de la migración
2. Verificar que el autoguardado funciona correctamente
3. Probar el sistema de traducciones
4. Verificar la persistencia de datos
5. Probar en diferentes navegadores (modo web)

---

**Fecha de migración**: 2026-02-04  
**Versión origen**: Flet 0.28.3  
**Versión destino**: Flet 0.80.0 (1.0 Beta)
