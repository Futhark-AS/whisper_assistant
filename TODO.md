**Latency between triggering record and recording starting**

It takes a while. The yellow indicator in the menu bar also lies about when the recording actually starts, if you start speaking just as the indicator turns yellow, your first words will not be included in the recording.

Alternatives to PyAudio
Library	Backend	Startup Speed	Notes
sounddevice	PortAudio	Similar to PyAudio	Cleaner API, same underlying engine
SoundCard	Native APIs (CoreAudio on macOS)	Potentially faster	Pure Python, uses OS-native APIs directly
rtmixer	PortAudio + C callback	Lower latency during recording	C callback avoids Python GIL, but still PortAudio init
PyObjC + AVFoundation	Native macOS	Likely fastest	Direct Apple API, no abstraction layer