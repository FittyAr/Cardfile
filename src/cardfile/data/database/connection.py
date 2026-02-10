from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker, scoped_session
from cardfile.config.config import Config


def get_engine():
    config = Config()
    uri = config.get_database_uri()
    return create_engine(uri, echo=config.get("app.debug", False))


def get_session():
    engine = get_engine()
    session_factory = sessionmaker(bind=engine)
    return scoped_session(session_factory)()
