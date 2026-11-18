// The Web Worker that reads a Notenübersicht (folia/crates/transcript): „Mein Studium" starts it for
// one file the visitor picked (`window.betulaTranscript` in folia/assets/boot.js), hands it the file
// and ends it once it has answered. The file is read here, in this worker's memory, and nowhere
// else: nothing is sent anywhere, nothing is stored, and the worker's memory goes when it is ended.
// It fetches nothing but its own WASM (src/wasm.rs), from the build of the page that started it.
//
// Message: the File → answer: the lines of `Transcript::lines` (or `error\t<code>`), or null where
// it could not read at all (its WASM did not load: offline before it was ever loaded).
//
// A plain (not module) worker: no imports, so it runs in every browser that has workers.

onmessage = async ({ data: file }) => {
  let answer = null;
  try {
    const [wasm, bytes] = await Promise.all([
      fetch(new URL(`transcript.wasm${self.location.search}`, self.location.href)).then((response) => {
        if (!response.ok) throw new Error(`transcript.wasm: HTTP ${response.status}`);
        return response.arrayBuffer();
      }),
      file.arrayBuffer(),
    ]);
    const x = (await WebAssembly.instantiate(wasm, {})).instance.exports;
    const pdf = new Uint8Array(bytes);
    const at = x.alloc(pdf.length);
    new Uint8Array(x.memory.buffer, at, pdf.length).set(pdf);
    const length = x.read(at, pdf.length);
    answer = new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.answer(), length));
  } catch (error) {
    console.info("[transcript] not read:", error);
  }
  postMessage(answer);
};
