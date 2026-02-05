# Dockerfile para Cardfile-py
# Usa Python 3.12 como base
FROM python:3.12-slim

# Establecer directorio de trabajo
WORKDIR /app

# Instalar dependencias del sistema si son necesarias
RUN apt-get update && apt-get install -y \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Copiar archivo de dependencias
COPY requirements.txt .

# Instalar dependencias de Python
RUN pip install --no-cache-dir --upgrade pip --root-user-action=ignore && \
    pip install --no-cache-dir -r requirements.txt --root-user-action=ignore

# Copiar el resto de la aplicación
COPY . .

# Crear directorio para la base de datos si no existe
RUN mkdir -p /app/data

# Exponer puerto para modo web de Flet
EXPOSE 8550

# Variables de entorno para modo web
ENV FLET_SERVER_PORT=8550
ENV FLET_SERVER_IP=0.0.0.0
ENV FLET_FORCE_WEB_SERVER=true

# Comando para ejecutar la aplicación en modo web
CMD ["flet", "run", "--web", "--port", "8550", "--host", "0.0.0.0", "main.py"]
