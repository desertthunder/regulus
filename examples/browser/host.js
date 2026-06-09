const textDecoder = new TextDecoder();

const TAG = {
  STRING: 1,
  LIST_CONS: 2,
  TUPLE: 3,
  RECORD: 4,
  CUSTOM: 5,
  CLOSURE: 6,
  BIT_ARRAY: 7,
  OPAQUE: 8,
  ERROR: 9,
  PANIC: 10,
};

export async function instantiateRegulus(source, options = {}) {
  let instance;
  const imports = createRegulusBrowserImports(() => instance, options);
  const result = await instantiate(source, imports);
  instance = result.instance;
  return result;
}

export function createRegulusBrowserImports(getInstance, options = {}) {
  const write = options.write ?? ((text) => console.log(text));
  const debug = options.debug ?? ((value) => console.debug(value));

  const memory = () => {
    const instance = typeof getInstance === "function" ? getInstance() : getInstance;
    if (!instance?.exports?.memory) {
      throw new Error("Regulus instance memory is not available");
    }
    return instance.exports.memory;
  };

  const readString = (ptr) => readRegulusString(memory(), ptr);
  const readDebugValue = (ptr) => inspectRegulusValue(memory(), ptr);

  return {
    browser: {
      print(ptr) {
        write(readString(ptr));
      },
      println(ptr) {
        write(`${readString(ptr)}\n`);
      },
      debug_i64(value) {
        debug(value.toString());
      },
      debug_f64(value) {
        debug(value);
      },
      debug_bool(value) {
        debug(value !== 0);
      },
      debug_value(ptr) {
        debug(readDebugValue(ptr));
      },
    },
  };
}

export function readRegulusString(memory, ptr) {
  const view = new DataView(memory.buffer);
  const tag = view.getUint32(ptr, true);
  if (tag !== TAG.STRING) {
    throw new Error(`expected string at ${ptr}, found tag ${tag}`);
  }
  const len = view.getUint32(ptr + 4, true);
  const bytes = new Uint8Array(memory.buffer, ptr + 8, len);
  return textDecoder.decode(bytes);
}

export function inspectRegulusValue(memory, ptr) {
  if (ptr === 0) {
    return "Nil";
  }

  const view = new DataView(memory.buffer);
  const tag = view.getUint32(ptr, true);
  const size = view.getUint32(ptr + 4, true);

  switch (tag) {
    case TAG.STRING:
      return JSON.stringify(readRegulusString(memory, ptr));
    case TAG.LIST_CONS:
      return `ListCons(size: ${size}, ptr: ${ptr})`;
    case TAG.TUPLE:
      return `Tuple(size: ${size}, ptr: ${ptr})`;
    case TAG.RECORD:
      return `Record(size: ${size}, ptr: ${ptr})`;
    case TAG.CUSTOM: {
      const constructor = view.getUint32(ptr + 8, true);
      return `Custom#${constructor}(size: ${size}, ptr: ${ptr})`;
    }
    case TAG.CLOSURE: {
      const functionId = view.getUint32(ptr + 8, true);
      return `Closure#${functionId}(captures: ${size}, ptr: ${ptr})`;
    }
    case TAG.BIT_ARRAY:
      return `BitArray(bits: ${size}, ptr: ${ptr})`;
    case TAG.OPAQUE:
      return `Opaque(ptr: ${ptr})`;
    case TAG.ERROR:
      return `Error(size: ${size}, ptr: ${ptr})`;
    case TAG.PANIC:
      return `Panic(size: ${size}, ptr: ${ptr})`;
    default:
      return `Unknown(tag: ${tag}, ptr: ${ptr})`;
  }
}

async function instantiate(source, imports) {
  if (source instanceof Response) {
    return WebAssembly.instantiateStreaming(source, imports);
  }
  if (typeof source === "string" || source instanceof URL) {
    return WebAssembly.instantiateStreaming(fetch(source), imports);
  }
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    return WebAssembly.instantiate(source, imports);
  }
  throw new TypeError("source must be a URL, Response, ArrayBuffer, or view");
}
