package counter

import (
	"io"
	"os"
	"path/filepath"
	"testing"
)

// trace is the trace file name in the directory CUPEL_TRACE_DIR names, when it is set.
func trace(t *testing.T, name string) io.Writer {
	dir := os.Getenv("CUPEL_TRACE_DIR")
	if dir == "" {
		return nil
	}
	file, err := os.Create(filepath.Join(dir, name))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { file.Close() })
	return file
}

func TestCountsThenFinishes(t *testing.T) {
	c := New(trace(t, "counts.jsonl"))
	c.Inc()
	c.Inc()
	c.Finish()
	if c.Count() != 2 {
		t.Fatal(c.Count())
	}
}

func TestBumps(t *testing.T) {
	c := New(trace(t, "bumps.jsonl"))
	c.Inc()
	c.Bump()
	if c.Count() != 3 {
		t.Fatal(c.Count())
	}
}
