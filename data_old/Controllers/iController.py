from abc import ABC, abstractmethod

class iController(ABC):
  @abstractmethod
  def __init__(self):
    # Constructor implementation
    pass
  
  @abstractmethod
  def Get(self):
    pass

  @abstractmethod
  def Update(self):
    pass

  @abstractmethod
  def Edit(self):
    pass

  @abstractmethod
  def Delete(self):
    pass

  @abstractmethod
  def Save(self):
    pass

# class ConcreteClass(DbController):
#  def Get(self):
#    # Implementación del método get
#    pass

#  def Update(self):
#    # Implementación del método update
#    pass

#  def Edit(self):
#    # Implementación del método edit
#    pass

#  def Delete(self):
#    # Implementación del método delete
#    pass

#  def Save(self):
#    # Implementación del método save
#    pass