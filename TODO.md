# UX / usability

**Latency between triggering record and recording starting**

It takes a while. The yellow indicator in the menu bar also lies about when the recording actually starts, if you start speaking just as the indicator turns yellow, your first words will not be included in the recording.

Alternatives to PyAudio
Library	Backend	Startup Speed	Notes
sounddevice	PortAudio	Similar to PyAudio	Cleaner API, same underlying engine
SoundCard	Native APIs (CoreAudio on macOS)	Potentially faster	Pure Python, uses OS-native APIs directly
rtmixer	PortAudio + C callback	Lower latency during recording	C callback avoids Python GIL, but still PortAudio init
PyObjC + AVFoundation	Native macOS	Likely fastest	Direct Apple API, no abstraction layer

**Streaming in transcription when pasting to cursor**

When pasting text to cursor, the transcription can be streamed in realtime / we can stop and start transcription in chunks so chunks can be pasted in realtime.

# Bugs

Sometimes it seems like the input monitoring janks up. We need better observability into what it has tracked to debug when this happens. Would be nice to have a log button just for what registered keys are registered as clicked down and released, with timestamps. 