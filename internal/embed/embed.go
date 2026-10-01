// Package embed computes the vectors of module passages for the semantic search with the
// encoder of Folia's crate semantic/, compiled to WebAssembly (semantic.wasm, the SIMD build of
// scripts/build-semantic.sh, which the browser's search worker runs too) and run in wazero, a
// WebAssembly runtime in pure Go. So the model exists once, in Rust, for Radix, the web server
// and the browser, and the vectors Radix publishes are the bits the crate computes natively: its
// int8 arithmetic is defined to the bit on every build (semantic/README.md).
package embed

import (
	"context"
	"crypto/sha256"
	_ "embed"
	"encoding/hex"
	"errors"
	"fmt"
	"math"
	"os"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// The module of the crate: scripts/build-semantic.sh writes it, from semantic/src.
//
//go:embed semantic.wasm
var module []byte

// Encoder embeds passages with a packed model (the server's: e5-de-en-server.bin, 8 bit, 512
// positions). It runs several instances of the module, each holding the model, and is safe for
// concurrent use: a call takes a free instance, and waits while all are busy.
type Encoder struct {
	id        string
	dims      int
	runtime   wazero.Runtime
	instances chan *instance
}

type instance struct {
	mod                api.Module
	alloc, free, embed api.Function
	packed, scale      uint32 // the results' room in the module's memory
}

// Load reads the packed model at path and starts workers instances of the module with it. Each
// takes about 80 MB of the process's memory once loaded, more while it loads, and a second to
// start. Radix runs one in each embed-worker process (Processes), none in its own.
func Load(ctx context.Context, path string, workers int) (*Encoder, error) {
	model, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	sum := sha256.Sum256(model)
	e := &Encoder{id: hex.EncodeToString(sum[:8]), instances: make(chan *instance, max(1, workers))}
	e.runtime = wazero.NewRuntime(ctx)
	compiled, err := e.runtime.CompileModule(ctx, module)
	if err != nil {
		_ = e.runtime.Close(ctx)
		return nil, fmt.Errorf("semantic.wasm: %w", err)
	}
	for i := range cap(e.instances) {
		inst, dims, err := start(ctx, e.runtime, compiled, model, i)
		if err != nil {
			_ = e.runtime.Close(ctx)
			return nil, fmt.Errorf("%s: %w", path, err)
		}
		e.dims = dims
		e.instances <- inst
	}
	return e, nil
}

func start(ctx context.Context, rt wazero.Runtime, compiled wazero.CompiledModule, model []byte, n int) (*instance, int, error) {
	mod, err := rt.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithName(fmt.Sprintf("semantic-%d", n)))
	if err != nil {
		return nil, 0, err
	}
	inst := &instance{mod: mod, alloc: mod.ExportedFunction("alloc"), free: mod.ExportedFunction("free"), embed: mod.ExportedFunction("embed_passage")}
	load := mod.ExportedFunction("load_encoder")
	if inst.alloc == nil || inst.free == nil || inst.embed == nil || load == nil {
		return nil, 0, errors.New("semantic.wasm lacks the encoder's exports; build it again (scripts/build-semantic.sh)")
	}
	at, err := inst.put(ctx, model)
	if err != nil {
		return nil, 0, err
	}
	// The module takes the model's bytes over: no free.
	res, err := load.Call(ctx, uint64(at), uint64(len(model)))
	if err != nil {
		return nil, 0, err
	}
	dims := int(int32(res[0]))
	if dims <= 0 {
		return nil, 0, errors.New("not a packed model")
	}
	if inst.packed, err = inst.allocate(ctx, dims/2); err != nil {
		return nil, 0, err
	}
	if inst.scale, err = inst.allocate(ctx, 4); err != nil {
		return nil, 0, err
	}
	return inst, dims, nil
}

// ID identifies the model: a vector is only comparable with vectors of the same model.
func (e *Encoder) ID() string { return e.id }

// Dims is the number of values of a vector; it takes Dims/2 bytes.
func (e *Encoder) Dims() int { return e.dims }

// EmbedPassage computes the vector of a module's passage (semantic.Passage) as the snapshot
// publishes it (semantic::quantize): its values of 4 bits, two to a byte, and their scale.
func (e *Encoder) EmbedPassage(ctx context.Context, passage string) (float32, []byte, error) {
	var inst *instance
	select {
	case inst = <-e.instances:
	case <-ctx.Done():
		return 0, nil, ctx.Err()
	}
	defer func() { e.instances <- inst }()

	at, err := inst.put(ctx, []byte(passage))
	if err != nil {
		return 0, nil, err
	}
	res, err := inst.embed.Call(ctx, uint64(at), uint64(len(passage)), uint64(inst.packed), uint64(inst.scale))
	if _, ferr := inst.free.Call(ctx, uint64(at), uint64(len(passage))); err == nil {
		err = ferr
	}
	if err != nil {
		return 0, nil, err
	}
	if int32(res[0]) < 0 {
		return 0, nil, errors.New("the passage is not UTF-8")
	}
	raw, ok := inst.mod.Memory().Read(inst.packed, uint32(e.dims/2))
	bits, ok2 := inst.mod.Memory().ReadUint32Le(inst.scale)
	if !ok || !ok2 {
		return 0, nil, errors.New("the vector lies outside the module's memory")
	}
	// Read returns a view of the module's memory: the next call overwrites it.
	return math.Float32frombits(bits), append([]byte(nil), raw...), nil
}

// Close stops the instances.
func (e *Encoder) Close(ctx context.Context) error {
	return e.runtime.Close(ctx)
}

// put copies data into the module's memory, at a place alloc gave out.
func (inst *instance) put(ctx context.Context, data []byte) (uint32, error) {
	at, err := inst.allocate(ctx, len(data))
	if err == nil && !inst.mod.Memory().Write(at, data) {
		err = errors.New("alloc gave out room outside the module's memory")
	}
	return at, err
}

func (inst *instance) allocate(ctx context.Context, n int) (uint32, error) {
	res, err := inst.alloc.Call(ctx, uint64(n))
	if err != nil {
		return 0, err
	}
	return uint32(res[0]), nil
}
