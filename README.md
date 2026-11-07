# tree-printer-directory

Renders a directory tree as a formatted text string using box-drawing characters.

```rust
use std::path::Path;
use tree_printer_directory::{render_tree, TreeOptions, TreeNode};

// Read a real directory and format it in one call.
let text = render_tree(Path::new("./src")).unwrap();
println!("{}", text);

// Or build a tree by hand and render it with custom options.
let mut root = TreeNode {
    name: String::from("project"),
    is_dir: true,
    children: Vec::new(),
};
root.push_child(TreeNode {
    name: String::from("lib.rs"),
    is_dir: false,
    children: Vec::new(),
});

let opts = TreeOptions {
    dir_suffix: String::from("/"),
    ..TreeOptions::default()
};
let mut out = String::new();
root.render(&mut out, &opts);
assert_eq!(out, "project/\n\u{2514}\u{2500}\u{2500} lib.rs\n");
```

## Why this exists

The standard library gives you `std::fs::read_dir`, but turning a directory
listing into something readable is a small pile of fiddly string work:
choosing connectors, tracking whether a parent was the last of its siblings,
and deciding how to order entries. This crate packages that work behind two
entry points so you do not reimplement it in every tool that prints a tree.

The trade-off is scope. There is no globbing, no filtering, no colour, and
no attempt to match the exact output of any external `tree` command. The
ordering rule is fixed: directories before files, then byte order. If you
want different ordering or filtering, build a `TreeNode` yourself and call
`render`; the rendering logic is independent of the filesystem walk.

## The edge you will hit

Symlinks are **not** followed. A symlink to a directory is reported as a
file, because following links can produce cycles and an unbounded render.
If you need to descend through symlinks, resolve them before calling
`read_tree`.

## API

- `TreeNode` — a node with `name`, `is_dir`, and `children`.
  - `TreeNode::render(&self, out: &mut String, opts: &TreeOptions)`
  - `TreeNode::push_child(&mut self, child: TreeNode)`
  - `TreeNode::sort_children(&mut self)`
- `TreeOptions` — `root_prefix`, `entry_prefix`, `dir_suffix`, `max_depth`.
  - `TreeOptions::default()`
- `read_tree(path: &Path) -> io::Result<TreeNode>`
- `render_tree(path: &Path) -> io::Result<String>`
- `render_tree_with(path: &Path, opts: &TreeOptions) -> io::Result<String>`
- `tree_from_entries(name: &str, entries: &[(String, bool)]) -> TreeNode`
- `child_path(base: &Path, name: &str) -> PathBuf`
