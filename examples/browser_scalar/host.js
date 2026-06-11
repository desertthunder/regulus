async function loadCounter(wasmUrl) {
  const bytes = await fetch(wasmUrl).then((response) => response.arrayBuffer());
  const module = await WebAssembly.instantiate(bytes, {});
  return module.instance.exports;
}

export { loadCounter };
