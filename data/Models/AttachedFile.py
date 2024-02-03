class AttachedFile(): 
    def __init__(self, Id: int, FileName: str, File: bytearray, UserId: str ):
        self.Id = Id
        self.FileName = FileName.strip()
        self.File = File
        self.UserId = UserId.strip()