import { abi, browserImportNames, call, callString, createBrowserImports, getHandle, init, releaseHandle, wrapHandle } from "./app.mjs";

if (abi.exports.describe_from_js.result !== "String") {
  throw new Error("missing export ABI metadata");
}
if (abi.imports["regulus/js.describe"].params.join(",") !== "Int,Float,Bool,String") {
  throw new Error("missing import ABI metadata");
}
if (abi.imports["regulus/js.pass_request"].params.join(",") !== "Handle") {
  throw new Error("missing handle import ABI metadata");
}
if (abi.exports.round_trip_request.result !== "Handle") {
  throw new Error("missing handle export ABI metadata");
}

const stored = new Map();
const browserImports = createBrowserImports({
  async fetch(url) {
    return { url };
  },
  localStorage: {
    getItem(key) {
      return stored.get(key) ?? null;
    },
    setItem(key, value) {
      stored.set(key, value);
    },
    removeItem(key) {
      stored.delete(key);
    },
  },
  now() {
    return 1234;
  },
  navigator: { onLine: false },
});
browserImports.browser[browserImportNames.localStorageSetItem]("name", "Ada");
if (browserImports.browser[browserImportNames.localStorageGetItem]("name") !== "Ada") {
  throw new Error("browser localStorage import did not read stored value");
}
browserImports.browser[browserImportNames.localStorageRemoveItem]("name");
if (browserImports.browser[browserImportNames.localStorageGetItem]("name") !== "") {
  throw new Error("browser localStorage import should map missing values to an empty string");
}
if (browserImports.browser[browserImportNames.timeNow]() !== 1234n) {
  throw new Error("browser time import returned an unexpected value");
}
if (browserImports.browser[browserImportNames.onlineIsOnline]() !== false) {
  throw new Error("browser online-state import returned an unexpected value");
}
const fetched = await browserImports.browser[browserImportNames.fetch]("https://example.test/");
if (fetched.url !== "https://example.test/") {
  throw new Error("browser fetch import did not call the configured fetch implementation");
}

const request = { url: "https://example.test/" };
await init(new URL("./app.wasm", import.meta.url), {
  "regulus/js": {
    request_text(input) {
      return `${input} from JS`;
    },
    describe(count, ratio, enabled, input) {
      return `${input}:${count}:${ratio}:${enabled}`;
    },
    pass_request(input) {
      if (input !== request) {
        throw new Error("opaque handle import did not resolve to the original JS object");
      }
      return input;
    },
  },
});

const result = callString("main", "hello");
if (result !== "hello from JS") {
  throw new Error(`unexpected result: ${result}`);
}

const described = call("describe_from_js", 7n, 2.5, true, "shape");
if (described !== "shape:7:2.5:true") {
  throw new Error(`unexpected described result: ${described}`);
}

const kept = call("keep_bool", true);
if (kept !== true) {
  throw new Error(`unexpected bool result: ${kept}`);
}

const response = call("response");
if (response.tag !== "Response" || response.fields.status !== 200n || response.fields.body !== "ok") {
  throw new Error(`unexpected response: ${response.tag}`);
}

const names = call("names");
if (names.join(",") !== "Ada,Joe") {
  throw new Error(`unexpected names: ${names}`);
}

const maybeName = call("maybe_name");
if (maybeName.tag !== "Some" || maybeName.value !== "Ada") {
  throw new Error(`unexpected option: ${JSON.stringify(maybeName)}`);
}

const resultName = call("result_name");
if (resultName.tag !== "Ok" || resultName.value !== "Ada") {
  throw new Error(`unexpected result: ${JSON.stringify(resultName)}`);
}

const roundTripped = call("round_trip_request", request);
if (roundTripped !== request) {
  throw new Error("opaque handle did not pass through Gleam");
}

const handle = wrapHandle(request, 42);
if (getHandle(handle, 42) !== request) {
  throw new Error("opaque handle did not round trip through the adapter table");
}
if (!releaseHandle(handle, 42)) {
  throw new Error("opaque handle was not released");
}
try {
  getHandle(handle, 42);
  throw new Error("released opaque handle lookup should fail");
} catch (error) {
  if (!String(error.message).includes("released")) {
    throw error;
  }
}
