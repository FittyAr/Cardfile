from sqlalchemy import Boolean, LargeBinary, create_engine, Column, Integer, String, ForeignKey, DateTime
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import relationship
from datetime import datetime

# Crear la conexión a la base de datos SQLite
engine = create_engine('sqlite:///database.db', echo=True)

# Crear una instancia de la clase base declarativa
Base = declarative_base()

# Definir el modelo de la tabla Card
class Card(Base):
    __tablename__ = 'Cards'
    
    Id = Column(Integer, primary_key=True, autoincrement=True)
    Name = Column(String)
    Text = Column(String)
    UserId = Column(Integer, ForeignKey('Users.Id'))
    CreatedAt = Column(DateTime, default=datetime.now)
    Lock = Column(Boolean, default=False)

    User = relationship('User', back_populates='CardFiles')
    attached_files = relationship('AttachedFile', back_populates='card')
    
    def __str__(self):
        return f'{self.Name}'

# Definir el modelo de la tabla User
class User(Base):
    __tablename__ = 'Users'

    Id = Column(Integer, primary_key=True)
    Name = Column(String)
    UserName = Column(String, nullable=False, unique=True)
    Normalized_UserName = Column(String, nullable=False, unique=True)
    Password = Column(String)
    Email = Column(String, nullable=False, unique=True)
    Normalized_Email = Column(String, nullable=False, unique=True)
    CreatedAt = Column(DateTime, default=datetime.now)

    card_files = relationship('Card', back_populates='User')
    
    def __str__(self):
        return f'{self.Name}'

# Definir el modelo de la tabla AttachedFile
class AttachedFile(Base):
    __tablename__ = 'AttachedFiles'

    Id = Column(Integer, primary_key=True)
    FileName = Column(String)
    File = Column(LargeBinary)
    CardId = Column(Integer, ForeignKey('Cards.Id'))

    card = relationship('Card', back_populates='attached_files')
    
    def __str__(self):
        return f'{self.FileName}'

# Crear las tablas en la base de datos
Base.metadata.create_all(engine)