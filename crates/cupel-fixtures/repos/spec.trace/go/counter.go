package counter

import (
	"encoding/json"
	"io"
	"runtime"
)

// Counter counts up and logs each step it takes to a trace, when it has one.
type Counter struct {
	count int
	done  bool
	trace io.Writer
}

func New(trace io.Writer) *Counter {
	c := &Counter{trace: trace}
	c.log("")
	return c
}

func (c *Counter) Inc() {
	c.count++
	c.log("Inc")
}

func (c *Counter) Bump() {
	c.count += 2
	c.log("Inc")
}

func (c *Counter) Finish() {
	c.done = true
	c.log("Finish")
}

func (c *Counter) Count() int {
	return c.count
}

func (c *Counter) log(action string) {
	if c.trace == nil {
		return
	}
	_, file, line, _ := runtime.Caller(1)
	step := map[string]any{
		"state": map[string]any{"count": c.count, "done": c.done},
		"file":  file,
		"line":  line,
	}
	if action != "" {
		step["action"] = action
	}
	text, _ := json.Marshal(step)
	c.trace.Write(append(text, '\n'))
}
