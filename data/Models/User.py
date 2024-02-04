import uuid

from sqlalchemy import DateTime

# hashlib
#hashlib.sha256(b"El Libro De Python").hexdigest()   

class User():
    def __init__ (Name: str, UserName: str,Password: str, Email: str, self, CreatedAt: DateTime, Id: uuid.UUID = None):
        self.__Id = Id if Id is not None else uuid.uuid4()
        self.__Name = Name
        self.__UserName = UserName.strip()
        self.__Normalized_UserName = UserName.strip().upper()
        self.__Password = Password.strip()
        self.__Email = Email.strip()
        self.__Normalized_Email = Email.strip().upper()
        self.__CreatedAt = CreatedAt
        
        @property
        def Id(self):
            return self.__Id
        @property
        def Name(self):
            return self.__Name
        @property
        def UserName(self):
            return self.__UserName
        @property
        def Normalized_UserName(self):
            return self.__Normalized_UserName
        @property
        def Password(self):
            return self.__Password
        @property
        def Email(self):
            return self.__Email
        @property
        def Normalized_Email(self):
            return self.__Normalized_Email
        @property
        def CreatedAt(self):
            return self.__CreatedAt
        
        @Id.setter
        def Id(self, value: uuid.UUID):
            self.__Id = value
        @Name.setter
        def Name(self, value: str):
            self.__Name = value
        @UserName.setter
        def UserName(self, value: str):
            self.__UserName = value.strip()
        @Normalized_UserName.setter
        def Normalized_UserName(self, value: str):
            self.__Normalized_UserName = value.strip().upper()
        @Password.setter
        def Password(self, value: str):
            self.__Password = value.strip()
        @Email.setter
        def Email(self, value: str):
            self.__Email = value.strip()
        @Normalized_Email.setter
        def Normalized_Email(self, value: str):
            self.__Normalized_Email = value.strip().upper()
        @CreatedAt.setter
        def CreatedAt(self, value: DateTime):
            self.__CreatedAt = value