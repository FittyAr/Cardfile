class Card():
    def __init__(self, Id: int, Name: str, Text: str, UserId: str, Lock: bool = False):
        self.Id = Id
        self.Name = Name.strip()
        self.Text = Text.strip()
        self.UserId = UserId.strip()
        self.Lock = Lock