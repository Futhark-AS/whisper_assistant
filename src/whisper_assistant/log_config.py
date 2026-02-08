import logging
import logging.handlers

from whisper_assistant.paths import get_log_dir

# Setup logging with dual handlers: console (INFO) and files (DEBUG and INFO)
log_format = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
log_dir = get_log_dir()
debug_log_file = log_dir / "debug.log"
info_log_file = log_dir / "info.log"

# Configure root logger
root_logger = logging.getLogger()
root_logger.setLevel(logging.DEBUG)


# Filter to silence httpx/httpcore INFO logs from console only
class HttpxConsoleFilter(logging.Filter):
    """Filter to silence httpx/httpcore INFO logs from console."""

    def filter(self, record: logging.LogRecord) -> bool:
        if record.name in ("httpx", "httpcore") and record.levelno == logging.INFO:
            return False
        return True


# Console handler - INFO level and above
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.INFO)
console_handler.addFilter(HttpxConsoleFilter())
console_handler.setFormatter(logging.Formatter(log_format))

# Debug file handler - DEBUG level and above
# Rotates daily at midnight, no backups (only current day)
debug_file_handler = logging.handlers.TimedRotatingFileHandler(
    debug_log_file, when="midnight", interval=1, backupCount=0
)
debug_file_handler.setLevel(logging.DEBUG)
debug_file_handler.setFormatter(logging.Formatter(log_format))

# Info file handler - INFO level and above
# Rotates daily at midnight, no backups (only current day)
info_file_handler = logging.handlers.TimedRotatingFileHandler(
    info_log_file, when="midnight", interval=1, backupCount=0
)
info_file_handler.setLevel(logging.INFO)
info_file_handler.setFormatter(logging.Formatter(log_format))

# Add handlers to root logger
root_logger.addHandler(console_handler)
root_logger.addHandler(debug_file_handler)
root_logger.addHandler(info_file_handler)
