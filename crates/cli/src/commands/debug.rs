use std::fs;
use std::path::Path;
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};
use owo_colors::OwoColorize;
use serde_json::{Value, json};
use tree_sitter::Node;

use crate::{args::DebugOptions, echo};

/// Developer-facing compiler inspection command.
///
/// The debug command is intentionally read-only.
///
/// It loads one Gleam file and prints one or more intermediate compiler views.
pub struct Debugger<'a> {
    pub input: &'a Path,
    pub tree_sitter: bool,
    pub ast: bool,
    pub spans: bool,
    pub json: bool,
    pub no_color: bool,
}

impl<'a> Debugger<'a> {
    pub fn new(input: &'a Path, opts: DebugOptions) -> Self {
        let DebugOptions { ts, ast, spans, json, no_color } = opts;
        Self { input, tree_sitter: ts, ast, spans, json, no_color }
    }
}

impl Debugger<'_> {
    pub fn run(&self) -> ExitCode {
        if !self.tree_sitter && !self.ast {
            echo::error("select at least one debug view, for example `--ts` or `--ast`");
            return ExitCode::FAILURE;
        }
        if self.spans && !self.tree_sitter {
            echo::error("`--spans` currently applies to `--ts`; pass both flags together");
            return ExitCode::FAILURE;
        }

        let text = match fs::read_to_string(self.input) {
            Ok(text) => text,
            Err(error) => return echo::fail("read", self.input.display(), error),
        };
        let source = SourceFile::with_path(SourceFileId(0), self.input, text);
        let cst = match compiler_core::parse::parse(source.clone()) {
            Ok(cst) => cst,
            Err(diagnostics) => {
                return echo::fail_with_source_diagnostics("parse", self.input.display(), &diagnostics, &[source]);
            }
        };

        if self.json {
            return self.print_json(&cst);
        }

        if self.tree_sitter {
            if self.spans {
                println!("{}", self.heading("tree-sitter"));
                print_tree(cst.tree.root_node(), 0, None, !self.no_color);
            } else {
                println!("{}", cst.tree.root_node().to_sexp());
            }
        }

        if self.ast {
            let ast = match compiler_core::ast::build(&cst) {
                Ok(ast) => ast,
                Err(diagnostics) => {
                    return echo::fail_with_source_diagnostics(
                        "build AST",
                        self.input.display(),
                        &diagnostics,
                        std::slice::from_ref(&cst.source),
                    );
                }
            };
            println!("{}", self.heading("ast"));
            println!("{ast:#?}");
        }

        ExitCode::SUCCESS
    }

    fn print_json(&self, cst: &compiler_core::parse::ConcreteSyntaxTree) -> ExitCode {
        let mut output = serde_json::Map::new();
        output.insert("file".to_string(), json!(self.input.to_string_lossy().to_string()));

        if self.tree_sitter {
            let value = if self.spans {
                tree_json(cst.tree.root_node(), None)
            } else {
                json!({ "sexp": cst.tree.root_node().to_sexp() })
            };
            output.insert("tree_sitter".to_string(), value);
        }

        if self.ast {
            let ast = match compiler_core::ast::build(cst) {
                Ok(ast) => ast,
                Err(diagnostics) => {
                    return echo::fail_with_source_diagnostics(
                        "build AST",
                        self.input.display(),
                        &diagnostics,
                        std::slice::from_ref(&cst.source),
                    );
                }
            };
            output.insert("ast".to_string(), json!(format!("{ast:#?}")));
        }

        match serde_json::to_string_pretty(&Value::Object(output)) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => echo::fail("debug", "JSON output", error),
        }
    }

    fn heading(&self, text: &str) -> String {
        if self.no_color {
            format!("{text}:")
        } else {
            format!("{}", format!("{text}:").bright_magenta().bold())
        }
    }
}

fn print_tree(node: Node<'_>, depth: usize, field: Option<&str>, color: bool) {
    let indent = "  ".repeat(depth);
    let kind = if color { format!("{}", node.kind().bright_cyan()) } else { node.kind().to_string() };
    let field = field
        .map(|field| {
            if color {
                format!(" {}={}", "field".bright_black(), field.bright_yellow())
            } else {
                format!(" field={field}")
            }
        })
        .unwrap_or_default();
    println!(
        "{indent}{kind}{field} [{}..{}] {}:{}..{}:{}",
        node.start_byte(),
        node.end_byte(),
        node.start_position().row + 1,
        node.start_position().column + 1,
        node.end_position().row + 1,
        node.end_position().column + 1
    );

    let mut cursor = node.walk();
    for (index, child) in node.named_children(&mut cursor).enumerate() {
        let field = node.field_name_for_named_child(index as u32);
        print_tree(child, depth + 1, field, color);
    }
}

fn tree_json(node: Node<'_>, field: Option<&str>) -> Value {
    let mut cursor = node.walk();
    let children = node
        .named_children(&mut cursor)
        .enumerate()
        .map(|(index, child)| tree_json(child, node.field_name_for_named_child(index as u32)))
        .collect::<Vec<_>>();

    json!({
        "kind": node.kind(),
        "field": field,
        "named": node.is_named(),
        "start_byte": node.start_byte(),
        "end_byte": node.end_byte(),
        "start": {
            "line": node.start_position().row + 1,
            "column": node.start_position().column + 1,
        },
        "end": {
            "line": node.end_position().row + 1,
            "column": node.end_position().column + 1,
        },
        "children": children,
    })
}
