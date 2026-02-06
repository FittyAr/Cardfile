from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker, scoped_session
from cardfile.config.config import DATABASE_URI

engine = create_engine(DATABASE_URI, echo=True)
Session = scoped_session(sessionmaker(bind=engine))

def get_session():
    return Session()
