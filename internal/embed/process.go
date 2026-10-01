package embed

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"os/exec"
	"runtime/debug"
)

// Why processes: WebAssembly that wazero compiled runs as machine code Go cannot preempt, so a
// call holds its thread until it returns — up to 6 s for a long passage — and every garbage
// collection of the process waits for it. In Radix's own process that would stall its HTTP
// server (the snapshots Folia downloads, /status for the health check) for seconds. In a process
// of its own an encoder stalls nothing but itself.
//
// The protocol on a worker's stdin and stdout (little endian):
//
//	worker → once at start:  u32 dims (0: it could not load the model; then u32 length, message)
//	→ worker, a passage:     u32 length, UTF-8
//	worker → its vector:     u32 0, f32 scale, dims/2 bytes  — or u32 1, u32 length, the error

// Serve answers passages on in with their vectors on out until in ends: what a worker process
// runs (radix embed-worker). model is the packed model's path.
func Serve(ctx context.Context, model string, in io.Reader, out io.Writer) error {
	w := bufio.NewWriter(out)
	e, err := Load(ctx, model, 1)
	if err != nil {
		_ = writeError(w, 0, err)
		return errors.Join(err, w.Flush())
	}
	defer e.Close(ctx)
	// The model's bytes are in the module's memory now; Go's copy is garbage, 35 MB of it.
	debug.FreeOSMemory()
	if err := binary.Write(w, binary.LittleEndian, uint32(e.Dims())); err != nil {
		return err
	}
	if err := w.Flush(); err != nil {
		return err
	}
	r := bufio.NewReader(in)
	for {
		passage, err := readBlock(r)
		if errors.Is(err, io.EOF) {
			return nil
		}
		if err != nil {
			return err
		}
		scale, packed, err := e.EmbedPassage(ctx, string(passage))
		if err != nil {
			err = writeError(w, 1, err)
		} else {
			err = errors.Join(binary.Write(w, binary.LittleEndian, uint32(0)), binary.Write(w, binary.LittleEndian, math.Float32bits(scale)))
			if err == nil {
				_, err = w.Write(packed)
			}
		}
		if err == nil {
			err = w.Flush()
		}
		if err != nil {
			return err
		}
	}
}

func writeError(w io.Writer, status uint32, err error) error {
	msg := []byte(err.Error())
	return errors.Join(binary.Write(w, binary.LittleEndian, status), binary.Write(w, binary.LittleEndian, uint32(len(msg))), func() error { _, e := w.Write(msg); return e }())
}

func readBlock(r io.Reader) ([]byte, error) {
	var n uint32
	if err := binary.Read(r, binary.LittleEndian, &n); err != nil {
		return nil, err
	}
	if n > 1<<24 {
		return nil, fmt.Errorf("a block of %d bytes", n)
	}
	b := make([]byte, n)
	_, err := io.ReadFull(r, b)
	return b, err
}

// Processes embeds passages in worker processes, one passage at a time each, and is safe for
// concurrent use: a call takes a free worker and waits while all are busy. A worker that dies
// is started again for the next call.
type Processes struct {
	id      string
	dims    int
	command func() *exec.Cmd
	workers chan *worker
}

type worker struct {
	cmd *exec.Cmd
	in  io.WriteCloser
	out *bufio.Reader
	err error // why the worker could not be started; it is tried again on the next call
}

// StartProcesses starts n workers, each a process command makes: one that runs Serve with the
// model at path, such as `radix embed-worker <path>`. Each holds the model, about 170 MB of
// memory.
func StartProcesses(path string, n int, command func() *exec.Cmd) (*Processes, error) {
	model, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	sum := sha256.Sum256(model)
	if err := checkName(path, sum); err != nil {
		return nil, err
	}
	p := &Processes{id: hex.EncodeToString(sum[:8]), command: command, workers: make(chan *worker, max(1, n))}
	for range cap(p.workers) {
		w, dims := p.start()
		if w.err == nil && p.dims != 0 && dims != p.dims {
			w.stop()
			w = &worker{err: fmt.Errorf("encoder processes with %d and %d dims", p.dims, dims)}
		}
		if w.err != nil {
			p.Close()
			return nil, w.err
		}
		p.dims = dims
		p.workers <- w
	}
	return p, nil
}

// start starts a worker and reads its dims; a worker that failed carries the error.
func (p *Processes) start() (*worker, int) {
	cmd := p.command()
	cmd.Stderr = os.Stderr
	in, err := cmd.StdinPipe()
	if err != nil {
		return &worker{err: err}, 0
	}
	out, err := cmd.StdoutPipe()
	if err != nil {
		return &worker{err: err}, 0
	}
	if err := cmd.Start(); err != nil {
		return &worker{err: fmt.Errorf("starting an encoder process: %w", err)}, 0
	}
	w := &worker{cmd: cmd, in: in, out: bufio.NewReader(out)}
	var dims uint32
	if err := binary.Read(w.out, binary.LittleEndian, &dims); err != nil {
		w.stop()
		return &worker{err: fmt.Errorf("an encoder process ended at start: %w", err)}, 0
	}
	if dims == 0 {
		msg, _ := readBlock(w.out)
		w.stop()
		return &worker{err: fmt.Errorf("an encoder process: %s", msg)}, 0
	}
	return w, int(dims)
}

func (w *worker) stop() {
	if w.cmd == nil {
		return
	}
	_ = w.in.Close()
	_ = w.cmd.Process.Kill()
	_ = w.cmd.Wait()
	w.cmd = nil
}

// ID identifies the model: a vector is only comparable with vectors of the same model.
func (p *Processes) ID() string { return p.id }

// Dims is the number of values of a vector; it takes Dims/2 bytes.
func (p *Processes) Dims() int { return p.dims }

// EmbedPassage is Encoder.EmbedPassage in a worker process.
func (p *Processes) EmbedPassage(ctx context.Context, passage string) (float32, []byte, error) {
	var w *worker
	select {
	case w = <-p.workers:
	case <-ctx.Done():
		return 0, nil, ctx.Err()
	}
	if w.cmd == nil {
		var dims int
		if w, dims = p.start(); w.err == nil && dims != p.dims {
			w.stop()
			w = &worker{err: fmt.Errorf("an encoder process with %d dims, not %d", dims, p.dims)}
		}
		if w.err != nil {
			p.workers <- w
			return 0, nil, w.err
		}
	}
	scale, packed, err := w.embed(passage, p.dims)
	var refused *refusal
	if err != nil && !errors.As(err, &refused) {
		w.stop() // out of step with its pipe, or dead: a new one next time
	}
	p.workers <- w
	return scale, packed, err
}

// refusal is an error the worker answered with; the worker is fine.
type refusal struct{ msg string }

func (r *refusal) Error() string { return r.msg }

func (w *worker) embed(passage string, dims int) (float32, []byte, error) {
	if err := errors.Join(binary.Write(w.in, binary.LittleEndian, uint32(len(passage))), func() error { _, e := io.WriteString(w.in, passage); return e }()); err != nil {
		return 0, nil, fmt.Errorf("an encoder process: %w", err)
	}
	var status, bits uint32
	if err := binary.Read(w.out, binary.LittleEndian, &status); err != nil {
		return 0, nil, fmt.Errorf("an encoder process: %w", err)
	}
	if status != 0 {
		msg, err := readBlock(w.out)
		if err != nil {
			return 0, nil, fmt.Errorf("an encoder process: %w", err)
		}
		return 0, nil, &refusal{string(msg)}
	}
	packed := make([]byte, dims/2)
	if err := errors.Join(binary.Read(w.out, binary.LittleEndian, &bits), func() error { _, e := io.ReadFull(w.out, packed); return e }()); err != nil {
		return 0, nil, fmt.Errorf("an encoder process: %w", err)
	}
	return math.Float32frombits(bits), packed, nil
}

// Close stops the workers that are not busy. A busy one ends by itself: a worker ends when
// its stdin does, and so when Radix does.
func (p *Processes) Close() {
	for range cap(p.workers) {
		select {
		case w := <-p.workers:
			w.stop()
		default:
			return
		}
	}
}
