configs = {
    "translate": {
        "whisper_mode": "translate",
        "agent": None,
        "use_clipboard_input": False
    },
    "transcribe": {
        "whisper_mode": "transcribe",
        "agent": None,
        "use_clipboard_input": False
    },
    "gpt_with_input": {
        "whisper_mode": "translate",
        "agent": "gpt",
        "use_clipboard_input": True
    },
    "gpt_translate": {
        "whisper_mode": "translate",
        "agent": "gpt",
        "use_clipboard_input": False
    },
    "gpt_transcribe": {
        "whisper_mode": "transcribe",
        "agent": "gpt",
        "use_clipboard_input": False
    },
    "gpt_followup": {
        "whisper_mode": "transcribe",
        "agent": "gpt",
        "use_clipboard_input": False,
        "gpt_followup": True
    }
}