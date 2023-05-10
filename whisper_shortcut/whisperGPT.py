import threading
import rumps
from config import Config
from main import main

cfg = Config()
rumps.debug_mode(True)

class AwesomeStatusBarApp(rumps.App):
    def __init__(self):
        super(AwesomeStatusBarApp, self).__init__("")
        program_thread = threading.Thread(target=main, args=(self,))
        program_thread.start()
if __name__ == "__main__":
    AwesomeStatusBarApp().run()