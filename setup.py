from setuptools import setup

APP = ['whisperGPT.py']
DATA_FILES = []
OPTIONS = {
    'argv_emulation': True,
    'plist': {
        'LSUIElement': True,
    },
    'packages': ['rumps', 'cchardet', 'pynput', 'dotenv', 'pyperclip', 'openai', 'pyaudio', 'wave'],
    'includes': ['pynput', 'dotenv', 'pyperclip', 'openai', 'pyaudio', 'wave', 'charset-normalizer']

}

setup(
    app=APP,
    data_files=DATA_FILES,
    options={'py2app': OPTIONS},
    setup_requires=['py2app'],
)