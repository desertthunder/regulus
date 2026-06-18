import { callString, initNode } from "./app.mjs";

await initNode();

const result = callString("main", "hello");
if (result !== "hello") {
  throw new Error(`unexpected result: ${result}`);
}
