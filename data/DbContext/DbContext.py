from peewee import *

# Definición de los modelos
class Author(Model):
    name = CharField()

    class Meta:
        database = db

class Book(Model):
    title = CharField()
    author = ForeignKeyField(Author, backref='books')

    class Meta:
        database = db
        
# Crear las tablas en la base de datos
db.connect()
db.create_tables([Author, Book])

# Crear algunos datos de ejemplo
author1 = Author.create(name="Stephen King")
author2 = Author.create(name="J.K. Rowling")

book1 = Book.create(title="The Shining", author=author1)
book2 = Book.create(title="Harry Potter and the Philosopher's Stone", author=author2)

# Consultar datos
print("Libros de Stephen King:")
for book in author1.books:
    print(book.title)

print("Libros de J.K. Rowling:")
for book in author2.books:
    print(book.title)

# Cerrar la conexión con la base de datos
db.close()