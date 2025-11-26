import logging
import logging.handlers
import os

# Setup logging with dual handlers: console (INFO) and files (DEBUG and INFO)
log_format = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
log_dir = os.path.dirname(os.path.dirname(__file__))
debug_log_file = os.path.join(log_dir, "logs.debug.log")
info_log_file = os.path.join(log_dir, "logs.info.log")

# Configure root logger
root_logger = logging.getLogger()
root_logger.setLevel(logging.DEBUG)


# Filter to silence httpx/httpcore INFO logs from console only
class HttpxConsoleFilter(logging.Filter):
    def filter(self, record):
        # Allow all logs except INFO level from httpx and httpcore
        if record.name in ("httpx", "httpcore") and record.levelno == logging.INFO:
            return False
        return True


# Console handler - INFO level and above
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.INFO)
console_handler.addFilter(HttpxConsoleFilter())
console_handler.setFormatter(logging.Formatter(log_format))

# Debug file handler - DEBUG level and above
# Rotates daily at midnight and keeps only 1 backup (today + yesterday)
# Logs older than 1 day are automatically deleted
debug_file_handler = logging.handlers.TimedRotatingFileHandler(
    debug_log_file, when="midnight", interval=1, backupCount=0
)
debug_file_handler.setLevel(logging.DEBUG)
debug_file_handler.setFormatter(logging.Formatter(log_format))

# Info file handler - INFO level and above
# Rotates daily at midnight and keeps only 1 backup (today + yesterday)
# Logs older than 1 day are automatically deleted
info_file_handler = logging.handlers.TimedRotatingFileHandler(
    info_log_file, when="midnight", interval=1, backupCount=0
)
info_file_handler.setLevel(logging.INFO)
info_file_handler.setFormatter(logging.Formatter(log_format))

# Add handlers to root logger
root_logger.addHandler(console_handler)
root_logger.addHandler(debug_file_handler)
root_logger.addHandler(info_file_handler)
