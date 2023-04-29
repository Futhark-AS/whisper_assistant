class BaseAction:
    def __init__(self, name, description, action, shortcut, config):
        self.name = name
        self.description = description
        self.action = action
        self.shortcut = shortcut
        self.config = config

    def __str__(self):
        return self.name + " - " + self.description

    def __repr__(self):
        return self.name + " - " + self.description

    def __call__(self, input_text):
        self.action(input_text)