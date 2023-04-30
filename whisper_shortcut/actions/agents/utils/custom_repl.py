import sys
import os
from io import StringIO
from typing import Dict, Optional

from pydantic import BaseModel, Field
from langchain.utilities import BashProcess


class CustomREPL():
    """Simulates a standalone Python REPL."""

    globals: Optional[Dict] = Field(default_factory=dict, alias="_globals")
    locals: Optional[Dict] = Field(default_factory=dict, alias="_locals")

    def run(self, command: str) -> str:
        """Run command with own globals/locals and returns anything printed."""
        old_stdout = sys.stdout
        sys.stdout = mystdout = StringIO()
        try:
            file_name = "temp123.py"

            # remove ```python ... ```
            command = command.replace("```python", "")
            command = command.replace("```", "")
            with open(file_name, "w") as f:
                f.write(command)

            # execfile(file_name, self.globals, self.locals)
            # run bash command
            self.bash_process.run(f"py {file_name}")


            sys.stdout = old_stdout
            output = mystdout.getvalue()

            # os.remove(file_name)

        except Exception as e:
            sys.stdout = old_stdout
            output = str(e)
        return output

    def add_code(self, command: str) -> str:
        """Run command with own globals/locals and returns anything printed."""
        old_stdout = sys.stdout
        sys.stdout = mystdout = StringIO()
        try:
            file_name = "temp123.py"

            # remove ```python ... ```
            command = command.replace("```python", "")
            command = command.replace("```", "")
            with open(file_name, "a") as f:
                f.write(command)

            # execfile(file_name, self.globals, self.locals)
            # run bash command
            self.bash_process.run(f"py {file_name}")


            sys.stdout = old_stdout
            output = mystdout.getvalue()

            # os.remove(file_name)

        except Exception as e:
            sys.stdout = old_stdout
            output = str(e)
        return output
