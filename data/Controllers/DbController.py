from abc import ABC, abstractclassmethod

class DbController(ABC):
  @abstractclassmethod
  def __init__(self):
    # Constructor implementation
    pass
  
  @abstractclassmethod
  def Get(self):
    pass

  @abstractclassmethod
  def Update(self):
    pass

  @abstractclassmethod
  def Edit(self):
    pass

  @abstractclassmethod
  def Delete(self):
    pass

  @abstractclassmethod
  def Save(self):
    pass

class ConcreteClass(DbController):
  def Get(self):
    # Implementación del método get
    pass

  def Update(self):
    # Implementación del método update
    pass

  def Edit(self):
    # Implementación del método edit
    pass

  def Delete(self):
    # Implementación del método delete
    pass

  def Save(self):
    # Implementación del método save
    pass