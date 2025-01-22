import os
import shutil
from setuptools import setup, find_packages

# Remove existing build directory
build_dir = 'build'
if os.path.exists(build_dir):
    shutil.rmtree(build_dir)

def find_library(name, brew_path):
    lib_path = None
    for root, dirs, files in os.walk(brew_path):
        if f'{name}.dylib' in files:
            lib_path = os.path.join(root, f'{name}.dylib')
            break
    if not lib_path:
        raise ValueError(f"Could not find {name}.dylib. Please ensure it's installed via Homebrew.")
    return lib_path

# Find required libraries
libffi_path = find_library('libffi', '/opt/homebrew/Cellar/libffi')
libssl_path = find_library('libssl', '/opt/homebrew/Cellar/openssl@3')
libcrypto_path = find_library('libcrypto', '/opt/homebrew/Cellar/openssl@3')

APP = ['whisperGPT.py']
DATA_FILES = [
    ('config', ['config/config.py', 'config/shortcuts.py', 'config/actions_config.py']),
    ('audio', ['audio/audio_processing.py', 'audio/sounds.py', 'audio/speak.py']),
    ('actions', ['actions/BaseAction.py', 'actions/simple_gpt_action.py', 'actions/transcribe.py', 'actions/translate.py']),
    ('prompts', ['prompts/prompts.py']),
    ('', ['main.py', 'utils.py', '.env']),
]

required_packages = [
    'rumps', 'openai', 'pynput', 'pyaudio', 'pydub', 'simpleaudio', 'assemblyai', 'groq', 
    'langchain', 'langchain_openai', 'langchain_groq', 'charset_normalizer', 'chardet'
]

OPTIONS = {
    'argv_emulation': True,
    'packages': required_packages,
    'includes': required_packages,
    'excludes': ['PyQt6', 'PyInstaller'],
    'frameworks': [libffi_path, libssl_path, libcrypto_path],
    'resources': ['README.md', 'requirements.txt'],
    'plist': {
        'CFBundleName': 'WhisperGPT',
        'CFBundleShortVersionString': '1.0.0',
        'CFBundleVersion': '1.0.0',
        'CFBundleIdentifier': 'com.yourdomain.WhisperGPT',
        'NSHumanReadableCopyright': 'Copyright © 2023 Your Name',
        'NSHighResolutionCapable': True,
        'LSUIElement': True,
    },
}

setup(
    app=APP,
    name='WhisperGPT',
    data_files=DATA_FILES,
    options={'py2app': OPTIONS},
    setup_requires=['py2app'],
    install_requires=[
        'assemblyai',
        'groq',
        'langchain',
        'chardet',
        'charset-normalizer',
        'langchain-groq',
        'langchain-openai',
        'langsmith',
        'numpy',
        'openai',
        'playsound',
        'pooch',
        'PyAudio',
        'pycparser',
        'pydub',
        'pynput',
        'pyperclip',
        'python-dotenv',
        'regex',
        'requests',
        'rumps',
        'scikit-learn',
        'scipy',
        'simpleaudio',
        'tiktoken',
        'tqdm',
    ],
)