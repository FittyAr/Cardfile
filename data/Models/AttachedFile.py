class AttachedFile(): 
    def __init__(self, Id: int, FileName: str, File: bytearray, UserId: str ):
        self.__Id = Id
        self.__FileName = FileName.strip()
        self.__File = File
        self.__UserId = UserId.strip()
        
        @property
        def Id(self):
            return self.__Id
        @property
        def FileName(self):
            return self.__FileName
        @property
        def File(self):
            return self.__File
        @property
        def UserId(self):
            return self.__UserId
        
        @Id.setter
        def Id(self, value: int):
            self.__Id = value
        @FileName.setter
        def FileName(self, value: str):
            self.__FileName = value.strip()
        @File.setter
        def File(self, value: bytearray):
            self.__File = value
        @UserId.setter
        def UserId(self, value: str):
            self.__UserId = value.strip()