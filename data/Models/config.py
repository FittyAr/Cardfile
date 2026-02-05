from sqlalchemy import Column, Integer, String, Boolean, DateTime
from data.models.base import Base

class AppConfig(Base):
    __tablename__ = 'app_config'

    id = Column(Integer, primary_key=True)
    key = Column(String(50), unique=True, nullable=False)
    value = Column(String(255)) 
