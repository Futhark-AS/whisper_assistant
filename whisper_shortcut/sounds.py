import io
import threading
import simpleaudio as sa
from pydub import AudioSegment

class AudioPlayer:
    def __init__(self):
        self.play_obj = None
        self.speed_factor = 1.0

    def load_audio(self, file_path):
        audio = AudioSegment.from_file(file_path)
        audio = audio.set_channels(1)
        audio = audio.set_frame_rate(44100)

        # Apply the speed factor to the audio
        audio = audio.speedup(playback_speed=self.speed_factor)

        wav_buffer = io.BytesIO()
        audio.export(wav_buffer, format="wav")
        wav_buffer.seek(0)

        return sa.WaveObject.from_wave_file(wav_buffer)

    def play(self, wave_obj, on_finish=None):
        if self.play_obj is not None and self.play_obj.is_playing():
            self.play_obj.stop()

        def monitor_playback(play_obj):
            play_obj.wait_done()
            if on_finish is not None:
                on_finish()

        self.play_obj = wave_obj.play()
        monitor_thread = threading.Thread(target=monitor_playback, args=(self.play_obj,))
        monitor_thread.start()

    def stop(self):
        if self.play_obj is not None and self.play_obj.is_playing():
            self.play_obj.stop()

    def set_playback_speed(self, speed_factor):
        # Set the speed factor for playback
        self.speed_factor = speed_factor
