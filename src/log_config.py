import logging
import logging.handlers
import os

# Setup logging with dual handlers: console (INFO) and file (DEBUG)
log_format = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
log_file = os.path.join(os.path.dirname(os.path.dirname(__file__)), "logs.log")

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

# File handler - DEBUG level and above
# Rotates daily at midnight and keeps only 1 backup (today + yesterday)
# Logs older than 1 day are automatically deleted
file_handler = logging.handlers.TimedRotatingFileHandler(
    log_file, when="midnight", interval=1, backupCount=1
)
file_handler.setLevel(logging.DEBUG)
file_handler.setFormatter(logging.Formatter(log_format))

# Add handlers to root logger
root_logger.addHandler(console_handler)
root_logger.addHandler(file_handler)
