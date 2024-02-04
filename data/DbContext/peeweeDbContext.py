from peewee import *

# Define el modelo de la tabla Card
class Card(Model):
    # Define los campos de la tabla
    id = AutoField()
    name = CharField()
    text = TextField()
    user_id = ForeignKeyField(User, backref='cards')
    created_at = DateTimeField()
    lock = BooleanField()

    class Meta:
        database = db

# Define el modelo de la tabla User
class User(Model):
    # Define los campos de la tabla
    id = AutoField()
    name = CharField()
    username = CharField()
    normalized_username = CharField()
    password = CharField()
    email = CharField()
    normalized_email = CharField()
    created_at = DateTimeField()

    class Meta:
        database = db

# Define el modelo de la tabla AttachedFile
class AttachedFile(Model):
    # Define los campos de la tabla
    id = AutoField()
    filename = CharField()
    file = BlobField()
    card_file_id = ForeignKeyField(Card, backref='attached_files')

    class Meta:
        database = db

# Crea la base de datos SQLite
db = SqliteDatabase('database.db')

# Conecta a la base de datos
db.connect()

# Crea las tablas si no existen
db.create_tables([Card, User, AttachedFile])

# Agrega un usuario inicial
User.create(username='admin', password='password')

# Cierra la conexión a la base de datos
db.close()