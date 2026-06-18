import { abi, callString, createNodeImports, initNode, nodeImportNames } from "./app.mjs";

if (abi.imports["nodejs.env.get"].params.join(",") !== "String") {
  throw new Error("missing Node import ABI metadata");
}

const nodeImports = createNodeImports({
  env: { REGULUS_NODE_SMOKE: "hello from node" },
  now() {
    return 1234;
  },
});

if (nodeImports.nodejs[nodeImportNames.envGet]("REGULUS_NODE_SMOKE") !== "hello from node") {
  throw new Error("Node env import did not read configured value");
}
if (nodeImports.nodejs[nodeImportNames.timeNow]() !== 1234n) {
  throw new Error("Node time import returned an unexpected value");
}

await initNode(undefined, {}, {
  env: { REGULUS_NODE_SMOKE: "hello from node" },
  now() {
    return 1234;
  },
});

const result = callString("main", "REGULUS_NODE_SMOKE");
if (result !== "hello from node") {
  throw new Error(`unexpected result: ${result}`);
}
