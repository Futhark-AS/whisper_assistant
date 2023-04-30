import rumps
from main import main
import threading

rumps.debug_mode(True)

class AwesomeStatusBarApp(rumps.App):
    def __init__(self):
        super(AwesomeStatusBarApp, self).__init__("")
        program_thread = threading.Thread(target=main, args=(self,))
        program_thread.start()
if __name__ == "__main__":
    AwesomeStatusBarApp().run()