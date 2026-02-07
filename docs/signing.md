# Firma de ejecutables en Windows

En mis releases de Windows, SmartScreen y Smart App Control pueden bloquear ejecutables no firmados o con baja reputación. No existe un certificado gratuito que elimine ese bloqueo de forma fiable. Para evitarlo necesito un certificado de firma de código confiable emitido por una CA.

## Opciones reales

1. Certificado EV (recomendado)
   - Es más caro y con verificación estricta
   - Suele eliminar bloqueos desde el primer release

2. Certificado OV
   - Es más económico
   - Puede seguir bloqueado hasta que el binario gane reputación

3. Microsoft Store
   - Es una alternativa viable sin manejar certificados propios
   - Requiere empaquetado y proceso de publicación

## Firma automática en GitHub Actions

El workflow soporta firma opcional si defino estos secretos:

- CODESIGN_PFX_BASE64
- CODESIGN_PFX_PASSWORD

Formato esperado:

1. Genero el PFX con mi proveedor de certificados.
2. Lo convierto a base64:

```bash
certutil -encode firma.pfx firma.pfx.base64
```

3. Copio el contenido base64 a `CODESIGN_PFX_BASE64`.
4. Guardo la contraseña en `CODESIGN_PFX_PASSWORD`.

Si los secretos no existen, el build se publica sin firmar.

## Certificado auto‑firmado

Puedo crear un certificado auto‑firmado para pruebas internas, pero Windows seguirá mostrando advertencias en máquinas que no confíen en ese certificado. No es una solución para distribución pública.
