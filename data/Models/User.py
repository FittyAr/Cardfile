import uuid

# hashlib
#hashlib.sha256(b"El Libro De Python").hexdigest()   

class User():
    def __init__ (Name: str, Password: str, Email: str, self, Id: uuid.UUID = None):
        self.__Id = Id if Id is not None else uuid.uuid4()
        self.__Name = Name
        self.__Normalized_Name = Name.strip().upper()
        self.__Password = Password.strip()
        self.__Email = Email.strip()
        self.__Normalized_Email = Email.strip().upper()
        
        @property
        def Id(self):
            return self.__Id
        @property
        def Name(self):
            return self.__Name
        @property
        def Normalized_Name(self):
            return self.__Normalized_Name
        @property
        def Password(self):
            return self.__Password
        @property
        def Email(self):
            return self.__Email
        @property
        def Normalized_Email(self):
            return self.__Normalized_Email
        
        @Id.setter
        def Id(self, value: uuid.UUID):
            self.__Id = value
        @Name.setter
        def Name(self, value: str):
            self.__Name = value
        @Normalized_Name.setter
        def Normalized_Name(self, value: str):
            self.__Normalized_Name = value
        @Password.setter
        def Password(self, value: str):
            self.__Password = value.strip()
        @Email.setter
        def Email(self, value: str):
            self.__Email = value.strip()
        @Normalized_Email.setter
        def Normalized_Email(self, value: str):
            self.__Normalized_Email = value.strip().upper()