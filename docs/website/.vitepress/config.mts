import { type DefaultTheme, defineConfig } from "vitepress";

const sidebar: DefaultTheme.Config["sidebar"] = [
  {
    text: "Guide",
    items: [
      { text: "Overview", link: "/guide/" },
      { text: "Installation", link: "/guide/installation" },
      {
        text: "Usage",
        link: "/guide/usage/",
        items: [{ text: "CLI", link: "/guide/usage/cli" }],
      },
      { text: "Examples", link: "/guide/examples" },
    ],
  },
  {
    text: "Reference",
    items: [
      { text: "Overview", link: "/reference/" },
      { text: "Supported subset", link: "/reference/supported-subset" },
      { text: "CLI and outputs", link: "/reference/cli-and-build-outputs" },
      { text: "Project model", link: "/reference/project-model-and-modules" },
      { text: "Gleam syntax", link: "/reference/full-gleam-syntax" },
      { text: "Name resolution", link: "/reference/full-name-resolution" },
      {
        text: "Types and interfaces",
        link: "/reference/gleam-types-and-interfaces",
      },
      { text: "Type inference", link: "/reference/type-and-generic-inference" },
      { text: "Pattern matching", link: "/reference/pattern-matching" },
      { text: "Closures", link: "/reference/closures" },
    ],
  },
  {
    text: "Development",
    items: [
      { text: "Overview", link: "/development/" },
      { text: "Testing", link: "/development/testing" },
      { text: "Core IR", link: "/development/core-ir" },
      {
        text: "Runtime representation",
        link: "/development/runtime-representation",
      },
      { text: "Runtime memory", link: "/development/runtime-memory" },
      { text: "Wasm backend", link: "/development/wasm-backend-and-runtime" },
    ],
  },
  {
    text: "Project",
    items: [{ text: "Changelog", link: "/changelog" }],
  },
];

export default defineConfig({
  title: "Regulus",
  description: "Docs for Regulus (aka Reggie), the experimental Gleam to WebAssembly compiler.",
  cleanUrls: true,
  lastUpdated: true,
  markdown: {
    theme: {
      light: "catppuccin-latte",
      dark: "catppuccin-macchiato",
    },
  },
  themeConfig: {
    logo: "/favicon.svg",
    siteTitle: "Regulus",
    nav: [
      { text: "Guide", link: "/guide/" },
      { text: "Reference", link: "/reference/" },
      { text: "Development", link: "/development/" },
      { text: "Changelog", link: "/changelog" },
    ],
    sidebar,
    search: {
      provider: "local",
    },
    outline: {
      level: [2, 3],
    },
    socialLinks: [{ icon: "github", link: "https://github.com/desertthunder/regulus" }],
  },
  head: [["link", { rel: "icon", href: "/favicon.svg" }]],
});
