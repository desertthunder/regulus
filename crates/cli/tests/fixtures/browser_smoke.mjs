import { abi, callString, initBrowserPage } from "./app.mjs";

if (abi.imports["browser.localStorage.getItem"].params.join(",") !== "String") {
  throw new Error("missing browser import ABI metadata");
}
if (abi.exports.main.result !== "String") {
  throw new Error("missing browser export ABI metadata");
}

const stored = new Map([["name", "Ada"]]);

await initBrowserPage(
  new URL("./app.wasm", import.meta.url),
  {},
  {
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
  },
);

const result = callString("main", "name");
if (result !== "Ada") {
  throw new Error(`unexpected browser result: ${result}`);
}
