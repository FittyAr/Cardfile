import uuid

# hashlib
#hashlib.sha256(b"El Libro De Python").hexdigest()   

class User():
    Id = uuid.UUID.__new__()
    Name = str()
    Normalized_Name = Name.strip().upper()
    Password = str()
    Email = str()
    Normalized_Email = Email.strip().upper()