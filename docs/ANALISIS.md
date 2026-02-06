# Análisis Técnico del Proyecto Cardfile-py

## 1. Descripción General

**Cardfile-py** es una aplicación de gestión de fichas/tarjetas inspirada en el estilo de Windows 3.1. La aplicación permite a los usuarios crear, editar, eliminar y gestionar fichas de información con soporte para formato Markdown. Incluye un sistema de autenticación, gestión de usuarios, y una papelera de reciclaje para recuperar elementos eliminados.

### 1.1 Características Principales

- **Sistema de Autenticación**: Login y registro de usuarios con encriptación de contraseñas usando bcrypt
- **Gestión de Fichas**: CRUD completo de fichas con título y descripción en Markdown
- **Editor Markdown**: Editor con preview en tiempo real y barra de herramientas
- **Papelera de Reciclaje**: Sistema de eliminación suave con posibilidad de restaurar
- **Multiidioma**: Soporte para 7 idiomas (Español, Inglés, Portugués BR, Francés, Alemán, Ruso, Chino)
- **Interfaz Moderna**: UI construida con Flet framework

## 2. Arquitectura del Sistema

### 2.1 Patrón Arquitectónico

El proyecto sigue un patrón **MVC (Modelo-Vista-Controlador)** con algunas variaciones:

- **Modelos**: Definidos en `data/Models/` usando SQLAlchemy ORM
- **Vistas**: Componentes de UI en `View/` usando Flet
- **Controladores**: Lógica de negocio distribuida entre vistas y repositorios
- **Repositorios**: Capa de acceso a datos en `data/repositories/`

### 2.2 Estructura de Directorios

```
Cardfile-py/
├── config/                 # Configuración de la aplicación
│   ├── config.py          # Gestión de configuración y traducciones
│   ├── Route.py           # Manejo de rutas y vistas
│   └── setup_db.py        # Inicialización de base de datos
├── data/                   # Capa de datos
│   ├── Models/            # Modelos SQLAlchemy
│   │   ├── base.py       # Clase base para modelos
│   │   ├── usuario.py    # Modelo Usuario
│   │   ├── ficha.py      # Modelo Ficha
│   │   └── config.py     # Modelo de configuración
│   ├── repositories/      # Repositorios de acceso a datos
│   │   ├── usuario_repository.py
│   │   └── ficha_repository.py
│   └── database/         # Configuración de base de datos
│       ├── connection.py # Conexión SQLAlchemy
│       └── setup.py      # Setup de tablas
├── View/                  # Componentes de interfaz de usuario
│   ├── Login.py          # Vista de login
│   ├── Card.py           # Vista principal de fichas
│   ├── NewCard.py        # Vista de nueva ficha
│   ├── EditCard.py       # Vista de edición de ficha
│   ├── Recycle.py        # Vista de papelera
│   ├── newUser.py        # Vista de registro
│   ├── Navigation.py     # Barra de navegación
│   └── MarkdownEditor.py # Editor Markdown
├── lang/                  # Archivos de traducción JSON
│   ├── es.json           # Español
│   ├── en.json           # Inglés
│   ├── pt_BR.json        # Portugués BR
│   ├── fr.json           # Francés
│   ├── de.json           # Alemán
│   ├── ru.json           # Ruso
│   └── zh.json           # Chino
├── main.py                # Punto de entrada de la aplicación
├── config.json           # Configuración JSON
├── requirements.txt      # Dependencias Python
└── README.md             # Documentación básica
```

## 3. Tecnologías Utilizadas

### 3.1 Framework Principal

- **Flet 0.28.3**: Framework Python para construir aplicaciones multiplataforma (Web, Desktop, Mobile)
  - **Estado**: Desactualizado (última versión: 0.80.0 - Flet 1.0 Beta)
  - **Uso**: Construcción de interfaz de usuario

### 3.2 Base de Datos

- **SQLAlchemy 2.0.43**: ORM para Python
  - **Motor**: SQLite (por defecto)
  - **Soporte**: MySQL, MariaDB, MsSQL (configurable)
  - **Modelos**: Usuario, Ficha

### 3.3 Seguridad

- **bcrypt 4.3.0**: Encriptación de contraseñas
  - Hash seguro con salt automático
  - Verificación de contraseñas en login

### 3.4 Otras Dependencias

- **markdown-it-py**: Renderizado de Markdown
- **MarkupSafe**: Escapado seguro de HTML
- **Pygments**: Resaltado de sintaxis para código
- **requests**: Cliente HTTP (si se requiere)
- **python-dotenv**: Gestión de variables de entorno

## 4. Flujo de Datos

### 4.1 Flujo de Autenticación

```
Usuario → Login View → Validación → UsuarioRepository → Base de Datos
                                    ↓
                              Verificación bcrypt
                                    ↓
                              Session Storage → Redirección a Card View
```

### 4.2 Flujo de Gestión de Fichas

```
Card View → Selección Ficha → Carga desde BD → Edición → Guardado
                                                          ↓
                                                    FichaRepository
                                                          ↓
                                                    Base de Datos
```

### 4.3 Flujo de Eliminación

```
Card View → Eliminar Ficha → Soft Delete (is_active=False) → Papelera
                                                                    ↓
                                                              Restaurar/Eliminar Permanentemente
```

## 5. Modelos de Datos

### 5.1 Modelo Usuario

```python
- id: Integer (PK)
- nombre: String(100)
- email: String(255) UNIQUE
- contraseña: String(255) [Hash bcrypt]
- is_active: Boolean
- last_login: DateTime
- created_at: DateTime
- updated_at: DateTime
- fichas: Relationship (One-to-Many con Ficha)
```

### 5.2 Modelo Ficha

```python
- id: Integer (PK)
- title: String(100)
- descripcion: String (Markdown)
- usuario_id: Integer (FK → Usuario.id)
- is_active: Boolean
- created_at: DateTime
- updated_at: DateTime
- usuario: Relationship (Many-to-One con Usuario)
```

## 6. Sistema de Traducciones

### 6.1 Implementación

- Archivos JSON por idioma en `lang/`
- Clase `Config` gestiona carga y cambio de idioma
- Preferencia guardada en `stored_language.txt`
- Cambio dinámico sin reiniciar aplicación

### 6.2 Idiomas Soportados

1. Español (es) - Por defecto
2. Inglés (en)
3. Portugués BR (pt_BR)
4. Francés (fr)
5. Alemán (de)
6. Ruso (ru)
7. Chino (zh)

## 7. Funcionalidades Detalladas

### 7.1 Sistema de Login

- Validación de campos vacíos
- Verificación de credenciales contra BD
- Hash de contraseña con bcrypt
- Actualización de `last_login`
- Almacenamiento de sesión en `client_storage` (Flet 0.28)
- Redirección automática según primera ejecución

### 7.2 Gestión de Fichas

- **Crear**: Formulario simple con título
- **Leer**: Lista de fichas con búsqueda en tiempo real
- **Cambiar nombre**: Modo lectura/edición con switch
- **Eliminar**: Soft delete a papelera
- **Autoguardado**: Debounce de 1 segundo + guardado periódico cada 15 segundos
- **Preview Markdown**: Vista previa en tiempo real

### 7.3 Editor Markdown

- Barra de herramientas con botones para:
  - Formato: Negrita, Cursiva, Tachado, Código inline/bloque
  - Encabezados: H1, H2, H3
  - Listas: Bullets, Numeradas, Checklist
  - Otros: Enlaces, Imágenes, Tablas, Citas, Regla horizontal
- Toggle para mostrar código fuente
- Preview con sanitización automática

### 7.4 Papelera de Reciclaje

- Lista de fichas eliminadas (`is_active=False`)
- Opción de restaurar (`is_active=True`)
- Eliminación permanente (DELETE físico)
- Confirmación antes de eliminar permanentemente

## 8. Problemas Identificados

### 8.1 Desactualización Crítica

1. **Flet 0.28.3 → 0.80.0**: Cambio mayor con breaking changes
   - `flet.app()` → `flet.run()`
   - `client_storage` → `shared_preferences`
   - `Icon.name` → `Icon.icon`
   - Botones: `text` → `content`
   - Uso de `AppView.WEB_BROWSER` para modo web

2. **Dependencias desactualizadas**: Muchos paquetes tienen versiones más recientes

### 8.2 Arquitectura

1. **Mezcla de responsabilidades**: Las vistas contienen lógica de negocio
2. **Manejo de sesiones**: No usa context managers para sesiones de BD
3. **Repositorios incompletos**: Algunos métodos no están implementados correctamente
4. **Sin logging estructurado**: Uso de `print()` en lugar de logging

### 8.3 Seguridad

1. **Validación de entrada**: Podría ser más robusta
2. **Sanitización**: Datos de usuario no se sanitizan antes de guardar
3. **Manejo de errores**: Excepciones genéricas sin logging adecuado

### 8.4 Performance

1. **Consultas N+1**: Posibles problemas en carga de relaciones
2. **Sin caché**: Traducciones y configuraciones se cargan repetidamente
3. **Autoguardado**: Podría optimizarse el debounce

### 8.5 Infraestructura

1. **Sin Docker**: No hay contenedorización para desarrollo/despliegue
2. **Sin variables de entorno**: Configuración hardcodeada
3. **Sin tests**: No hay suite de pruebas automatizadas

## 9. Propuestas de Mejora

### 9.1 Migración a Flet 1.0 Beta (Prioridad ALTA)

**Objetivo**: Actualizar a Flet 0.80.0 con todas las mejoras y breaking changes

**Cambios requeridos**:
- Migrar `flet.app()` a `flet.app(target=main, view=AppView.WEB_BROWSER)`
- Actualizar `client_storage` a `shared_preferences`
- Cambiar todas las propiedades obsoletas
- Actualizar manejo de diálogos
- Migrar eventos y handlers

**Beneficios**:
- Mejor rendimiento
- Soporte WebAssembly para web
- Arquitectura más moderna
- Mejor documentación y soporte

### 9.2 Dockerización (Prioridad ALTA)

**Objetivo**: Crear entorno Docker para desarrollo y producción

**Componentes**:
- Dockerfile con Python 3.12
- docker-compose.yml para orquestación
- Volúmenes para persistencia de BD
- Variables de entorno configurables

**Beneficios**:
- Entorno reproducible
- Fácil despliegue
- Aislamiento de dependencias
- Compatibilidad multiplataforma

### 9.3 Refactorización de Arquitectura (Prioridad MEDIA)

**Objetivo**: Separar mejor las responsabilidades

**Mejoras**:
- Crear capa de servicios para lógica de negocio
- Usar context managers para sesiones de BD
- Implementar repositorios completos con patrones adecuados
- Separar validaciones de UI

### 9.4 Mejoras de Seguridad (Prioridad MEDIA)

**Objetivo**: Fortalecer la seguridad de la aplicación

**Mejoras**:
- Validación exhaustiva de entrada
- Sanitización de datos antes de guardar
- Implementar rate limiting para login
- Agregar CSRF protection si aplica
- Mejorar manejo de errores sin exponer información sensible

### 9.5 Sistema de Logging (Prioridad BAJA)

**Objetivo**: Implementar logging estructurado

**Mejoras**:
- Reemplazar `print()` por logging
- Configurar niveles de log apropiados
- Logging de operaciones críticas
- Rotación de logs

### 9.6 Optimización de Performance (Prioridad BAJA)

**Objetivos**:
- Implementar caché para traducciones
- Optimizar consultas a BD con eager loading
- Lazy loading de vistas
- Optimizar autoguardado

### 9.7 Testing (Prioridad BAJA)

**Objetivo**: Implementar suite de pruebas

**Tipos de tests**:
- Unit tests para repositorios y servicios
- Integration tests para flujos completos
- UI tests para componentes críticos

## 10. Diagramas

### 10.1 Diagrama de Arquitectura

```
┌─────────────────────────────────────────────────────────┐
│                    Capa de Presentación                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │  Login   │  │   Card   │  │  NewCard │  ...        │
│  │   View   │  │   View   │  │   View   │            │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘            │
│       │            │              │                    │
└───────┼────────────┼──────────────┼────────────────────┘
        │            │              │
        └────────────┼──────────────┘
                     │
        ┌────────────▼────────────┐
        │   Capa de Control       │
        │  ┌──────────────────┐   │
        │  │   Route Handler  │   │
        │  │   Config Manager │   │
        │  └──────────────────┘   │
        └────────────┬─────────────┘
                     │
        ┌────────────▼────────────┐
        │   Capa de Negocio       │
        │  ┌──────────────────┐   │
        │  │  Repositories    │   │
        │  │  - UsuarioRepo   │   │
        │  │  - FichaRepo     │   │
        │  └──────────────────┘   │
        └────────────┬─────────────┘
                     │
        ┌────────────▼────────────┐
        │   Capa de Datos         │
        │  ┌──────────────────┐   │
        │  │   SQLAlchemy     │   │
        │  │   - Usuario      │   │
        │  │   - Ficha        │   │
        │  └──────────────────┘   │
        └────────────┬─────────────┘
                     │
        ┌────────────▼────────────┐
        │   Base de Datos         │
        │   SQLite / MySQL / etc  │
        └─────────────────────────┘
```

### 10.2 Flujo de Usuario Principal

```
Inicio
  │
  ├─→ ¿Primera ejecución?
  │     │
  │     ├─→ SÍ → Vista Registro Usuario
  │     │         │
  │     │         └─→ Crear Usuario → Vista Login
  │     │
  │     └─→ NO → Vista Login
  │               │
  │               └─→ Autenticación
  │                     │
  │                     ├─→ Fallida → Mostrar Error
  │                     │
  │                     └─→ Exitosa → Vista Card
  │                           │
  │                           ├─→ Nueva Ficha → Vista NewCard
  │                           │
  │                           ├─→ Cambiar nombre de ficha → Vista EditCard
  │                           │
  │                           ├─→ Eliminar → Soft Delete → Papelera
  │                           │
  │                           └─→ Papelera → Vista Recycle
  │                                 │
  │                                 ├─→ Restaurar
  │                                 │
  │                                 └─→ Eliminar Permanentemente
```

## 11. Consideraciones Técnicas

### 11.1 Base de Datos

- **SQLite por defecto**: Adecuado para desarrollo y uso individual
- **Soporte multi-SGBD**: Configurable para producción
- **Migraciones**: Actualmente manuales, considerar Alembic

### 11.2 Estado de la Aplicación

- **Sesión de usuario**: Almacenada en `client_storage` (Flet 0.28)
- **Ficha seleccionada**: Almacenada temporalmente en `client_storage`
- **Idioma**: Persistido en `stored_language.txt`

### 11.3 Autoguardado

- **Debounce**: 1 segundo después de última modificación
- **Guardado periódico**: Cada 15 segundos durante edición
- **Indicador visual**: Estado de guardado (Guardado/Cambios sin guardar/Guardando/Error)

## 12. Conclusión

Cardfile-py es una aplicación funcional con una arquitectura razonable pero que requiere actualización urgente a Flet 1.0 Beta y mejoras en infraestructura (Docker). El código base es mantenible pero se beneficiaría de refactorización para separar mejor las responsabilidades y mejorar la seguridad y performance.

### Prioridades de Implementación

1. **URGENTE**: Migración a Flet 1.0 Beta
2. **URGENTE**: Dockerización
3. **IMPORTANTE**: Actualización de dependencias
4. **IMPORTANTE**: Mejoras de seguridad
5. **RECOMENDADO**: Refactorización arquitectónica
6. **OPCIONAL**: Testing y optimizaciones

---

**Documento generado**: 2026-02-04  
**Versión del proyecto analizada**: Flet 0.28.3  
**Versión objetivo**: Flet 0.80.0 (1.0 Beta)
