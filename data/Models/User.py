import uuid

# hashlib
#hashlib.sha256(b"El Libro De Python").hexdigest()   

class User():
    def __init__ (Name: str, Password: str, Email: str, self, Id: uuid.UUID = None):
        self.Id = Id if Id is not None else uuid.uuid4()
        self.Name = Name
        self.Normalized_Name = Name.strip().upper()
        self.Password = Password.strip()
        self.Email = Email.strip()
        self.Normalized_Email = Email.strip().upper()