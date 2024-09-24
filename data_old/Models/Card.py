from sqlalchemy import DATETIME

class Card():
    def __init__(self, Id: int, Name: str, Text: str, UserId: str, CreatedAt: DATETIME, Lock: bool = False):
        self.__Id = Id
        self.__Name = Name.strip()
        self.__Text = Text.strip()
        self.__UserId = UserId.strip()
        self.__CreatedAt = CreatedAt
        self.__Lock = Lock
        
        @property
        def Id(self):
            return self.__Id
        @property
        def Name(self):
            return self.__Name
        @property
        def Text(self):
            return self.__Text
        @property
        def UserId(self):
            return self.__UserId
        @property
        def CreatedAt(self):
            return self.__CreatedAt
        @property
        def Lock(self):
            return self.__Lock
        
        @Id.setter
        def Id(self, value: int):
            self.__Id = value
        @Name.setter
        def Name(self, value: str):
            self.__Name = value.strip()
        @Text.setter
        def Text(self, value: str):
            self.__Text = value.strip()
        @UserId.setter
        def UserId(self, value: str):
            self.__UserId = value.strip()
        @CreatedAt.setter
        def CreatedAt(self, value: DATETIME):
            self.__CreatedAt = value
        @Lock.setter
        def Lock(self, value: bool):
            self.__Lock = value