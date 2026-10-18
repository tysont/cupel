import inspect
import json


class Counter:
    """Counts up and logs each step it takes to a trace, when it has one."""

    def __init__(self, trace=None):
        self.count = 0
        self.done = False
        self.trace = trace
        self._log(None)

    def inc(self):
        self.count += 1
        self._log("Inc")

    def bump(self):
        self.count += 2
        self._log("Inc")

    def finish(self):
        self.done = True
        self._log("Finish")

    def _log(self, action):
        if self.trace is None:
            return
        caller = inspect.stack()[1]
        step = {
            "state": {"count": self.count, "done": self.done},
            "file": caller.filename,
            "line": caller.lineno,
        }
        if action is not None:
            step["action"] = action
        self.trace.write(json.dumps(step) + "\n")
