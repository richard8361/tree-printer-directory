//! Render a directory tree as a formatted text string using box-drawing
//! characters.
//!
//! The library is deliberately tiny: it walks a real filesystem directory
//! and produces a string. It does not sort by anything exotic, it does not
//! follow symlinks (that way lies infinite loops), and it does not attempt
//! to render permissions or sizes. The one job is to turn a directory into
//! something that looks like the output of `tree(1)`.
//!
//! The tree is rendered depth-first, with directories listed before files
//! and each group sorted lexicographically by name. That ordering is stable
//! and easy to reason about, which matters more here than matching any
//! particular external tool byte-for-byte.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Options controlling how a tree is rendered.
#[derive(Debug, Clone)]
pub struct TreeOptions {
    /// Text drawn before the name of the root entry.
    pub root_prefix: String,
    /// Text drawn before the name of a non-root entry.
    pub entry_prefix: String,
    /// Text drawn before the name of a directory.
    pub dir_suffix: String,
    /// Maximum recursion depth. `0` means only the root is listed.
    pub max_depth: usize,
}

impl Default for TreeOptions {
    fn default() -> Self {
        TreeOptions {
            root_prefix: String::new(),
            entry_prefix: String::new(),
            dir_suffix: String::from("/"),
            max_depth: usize::MAX,
        }
    }
}

/// A single node in a rendered tree.
///
/// The struct is public so callers can inspect the structure before
/// formatting it, or format a subtree independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    /// The file name (not the full path).
    pub name: String,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// Children, already sorted, or empty for files.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Render this node and its descendants into `out` using `opts`.
    ///
    /// The root node is rendered with `root_prefix`; every other node uses
    /// `entry_prefix` plus the appropriate connector characters.
    pub fn render(&self, out: &mut String, opts: &TreeOptions) {
        let _ = write!(out, "{}{}", opts.root_prefix, self.name);
        if self.is_dir {
            let _ = write!(out, "{}", opts.dir_suffix);
        }
        out.push('\n');
        self.render_children(out, opts, 0);
    }

    fn render_children(&self, out: &mut String, opts: &TreeOptions, depth: usize) {
        if depth >= opts.max_depth {
            return;
        }
        let count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            let last = i + 1 == count;
            let connector = if last { "\u{2514}\u{2500}\u{2500} " } else { "\u{251c}\u{2500}\u{2500} " };
            let _ = write!(out, "{}{}{}", opts.entry_prefix, connector, child.name);
            if child.is_dir {
                let _ = write!(out, "{}", opts.dir_suffix);
            }
            out.push('\n');
            // The continuation prefix for grandchildren is the parent's
            // prefix plus either a pipe (if more siblings follow) or spaces.
            let mut child_opts = opts.clone();
            let continuation = if last { "    " } else { "\u{2502}   " };
            child_opts.entry_prefix = format!("{}{}", opts.entry_prefix, continuation);
            child.render_children(out, &child_opts, depth + 1);
        }
    }
}

/// Read a directory from the filesystem and build a [`TreeNode`].
///
/// Entries are sorted with directories first, then files, each group in
/// ascending byte order of the file name. Symlinks are treated as plain
/// files and are never followed, which keeps the render finite.
///
/// Returns an error if `path` cannot be read as a directory.
pub fn read_tree(path: &Path) -> io::Result<TreeNode> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let mut node = TreeNode {
        name,
        is_dir: true,
        children: Vec::new(),
    };
    read_children(path, &mut node)?;
    Ok(node)
}

fn read_children(path: &Path, node: &mut TreeNode) -> io::Result<()> {
    let mut entries: Vec<(String, bool)> = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // `file_type` uses the directory entry itself, so a symlink to a
        // directory reports as a symlink and is treated as a file. That is
        // deliberate: following symlinks can produce cycles.
        let is_dir = file_type.is_dir();
        entries.push((name, is_dir));
    }
    entries.sort_by(|a, b| {
        // Directories first, then files, then by name.
        b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
    });
    for (name, is_dir) in entries {
        if is_dir {
            let mut child = TreeNode {
                name,
                is_dir: true,
                children: Vec::new(),
            };
            read_children(&path.join(&child.name), &mut child)?;
            node.children.push(child);
        } else {
            node.children.push(TreeNode {
                name,
                is_dir: false,
                children: Vec::new(),
            });
        }
    }
    Ok(())
}

/// Render a directory tree rooted at `path` into a string.
///
/// This is the convenience entry point: it reads the tree and formats it
/// with default options in one call. Use [`read_tree`] plus
/// [`TreeNode::render`] if you need control over the options or want to
/// inspect the structure first.
pub fn render_tree(path: &Path) -> io::Result<String> {
    let tree = read_tree(path)?;
    let opts = TreeOptions::default();
    let mut out = String::new();
    tree.render(&mut out, &opts);
    Ok(out)
}

/// Render a directory tree rooted at `path` using explicit options.
pub fn render_tree_with(path: &Path, opts: &TreeOptions) -> io::Result<String> {
    let tree = read_tree(path)?;
    let mut out = String::new();
    tree.render(&mut out, opts);
    Ok(out)
}

/// Build a [`TreeNode`] from an explicit list of `(name, is_dir)` pairs.
///
/// This is useful for tests and for callers that already have a directory
/// listing (for example from a virtual filesystem) and do not want to touch
/// the real filesystem. The resulting node is a directory named `name`.
///
/// Note that this constructor does not recurse: every entry becomes a
/// child of the returned node with no children of its own. Use
/// [`TreeNode::push_child`] to build deeper structures.
pub fn tree_from_entries(name: &str, entries: &[(String, bool)]) -> TreeNode {
    let mut node = TreeNode {
        name: name.to_string(),
        is_dir: true,
        children: Vec::new(),
    };
    for (entry_name, is_dir) in entries {
        node.children.push(TreeNode {
            name: entry_name.clone(),
            is_dir: *is_dir,
            children: Vec::new(),
        });
    }
    node
}

impl TreeNode {
    /// Append a child node. The caller is responsible for keeping the
    /// children sorted if that matters to them; [`TreeNode::sort_children`]
    /// will re-sort in place.
    pub fn push_child(&mut self, child: TreeNode) {
        self.children.push(child);
    }

    /// Sort children directories-first, then by name.
    pub fn sort_children(&mut self) {
        self.children.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
        });
        for child in &mut self.children {
            child.sort_children();
        }
    }
}

/// Convenience: join a base path with a name, returning a [`PathBuf`].
///
/// This exists so callers do not have to import `std::path::PathBuf` just
/// to build a path from a directory and a child name.
pub fn child_path(base: &Path, name: &str) -> PathBuf {
    base.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> TreeNode {
        let mut root = TreeNode {
            name: String::from("root"),
            is_dir: true,
            children: Vec::new(),
        };
        let mut src = TreeNode {
            name: String::from("src"),
            is_dir: true,
            children: Vec::new(),
        };
        src.push_child(TreeNode {
            name: String::from("lib.rs"),
            is_dir: false,
            children: Vec::new(),
        });
        src.push_child(TreeNode {
            name: String::from("main.rs"),
            is_dir: false,
            children: Vec::new(),
        });
        root.push_child(src);
        root.push_child(TreeNode {
            name: String::from("Cargo.toml"),
            is_dir: false,
            children: Vec::new(),
        });
        root.push_child(TreeNode {
            name: String::from("README.md"),
            is_dir: false,
            children: Vec::new(),
        });
        root
    }

    #[test]
    fn renders_root_only() {
        let root = TreeNode {
            name: String::from("solo"),
            is_dir: true,
            children: Vec::new(),
        };
        let opts = TreeOptions::default();
        let mut out = String::new();
        root.render(&mut out, &opts);
        assert_eq!(out, "solo/\n");
    }

    #[test]
    fn renders_nested_tree() {
        let root = sample_tree();
        let opts = TreeOptions::default();
        let mut out = String::new();
        root.render(&mut out, &opts);
        let expected = "root/\n\
\u{251c}\u{2500}\u{2500} src/\n\
\u{2502}   \u{251c}\u{2500}\u{2500} lib.rs\n\
\u{2502}   \u{2514}\u{2500}\u{2500} main.rs\n\
\u{251c}\u{2500}\u{2500} Cargo.toml\n\
\u{2514}\u{2500}\u{2500} README.md\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn last_child_uses_spaces_for_continuation() {
        // Two directories, the second is last. Its children should be
        // indented with spaces, not a vertical bar.
        let mut root = TreeNode {
            name: String::from("r"),
            is_dir: true,
            children: Vec::new(),
        };
        let mut a = TreeNode {
            name: String::from("a"),
            is_dir: true,
            children: Vec::new(),
        };
        a.push_child(TreeNode {
            name: String::from("a1"),
            is_dir: false,
            children: Vec::new(),
        });
        let mut b = TreeNode {
            name: String::from("b"),
            is_dir: true,
            children: Vec::new(),
        };
        b.push_child(TreeNode {
            name: String::from("b1"),
            is_dir: false,
            children: Vec::new(),
        });
        root.push_child(a);
        root.push_child(b);
        let opts = TreeOptions::default();
        let mut out = String::new();
        root.render(&mut out, &opts);
        assert_eq!(out, "r/\n\u{251c}\u{2500}\u{2500} a/\n\u{2502}   \u{2514}\u{2500}\u{2500} a1\n\u{2514}\u{2500}\u{2500} b/\n    \u{2514}\u{2500}\u{2500} b1\n");
    }

    #[test]
    fn max_depth_zero_lists_only_root() {
        let root = sample_tree();
        let opts = TreeOptions {
            max_depth: 0,
            ..TreeOptions::default()
        };
        let mut out = String::new();
        root.render(&mut out, &opts);
        assert_eq!(out, "root/\n");
    }

    #[test]
    fn max_depth_one_lists_immediate_children() {
        let root = sample_tree();
        let opts = TreeOptions {
            max_depth: 1,
            ..TreeOptions::default()
        };
        let mut out = String::new();
        root.render(&mut out, &opts);
        let expected = "root/\n\
\u{251c}\u{2500}\u{2500} src/\n\
\u{251c}\u{2500}\u{2500} Cargo.toml\n\
\u{2514}\u{2500}\u{2500} README.md\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn custom_prefixes_are_used() {
        let root = TreeNode {
            name: String::from("x"),
            is_dir: true,
            children: Vec::new(),
        };
        let opts = TreeOptions {
            root_prefix: String::from("=> "),
            entry_prefix: String::from(".."),
            dir_suffix: String::from(""),
            max_depth: usize::MAX,
        };
        let mut out = String::new();
        root.render(&mut out, &opts);
        assert_eq!(out, "=> x\n");
    }

    #[test]
    fn dir_suffix_is_configurable() {
        let mut root = TreeNode {
            name: String::from("r"),
            is_dir: true,
            children: Vec::new(),
        };
        root.push_child(TreeNode {
            name: String::from("sub"),
            is_dir: true,
            children: Vec::new(),
        });
        let opts = TreeOptions {
            dir_suffix: String::from(" [dir]"),
            ..TreeOptions::default()
        };
        let mut out = String::new();
        root.render(&mut out, &opts);
        assert_eq!(out, "r [dir]\n\u{2514}\u{2500}\u{2500} sub [dir]\n");
    }

    #[test]
    fn tree_from_entries_builds_flat_tree() {
        let entries = vec![
            (String::from("a.txt"), false),
            (String::from("sub"), true),
        ];
        let node = tree_from_entries("top", &entries);
        assert_eq!(node.name, "top");
        assert!(node.is_dir);
        assert_eq!(node.children.len(), 2);
        assert_eq!(node.children[0].name, "a.txt");
        assert!(!node.children[0].is_dir);
        assert_eq!(node.children[1].name, "sub");
        assert!(node.children[1].is_dir);
    }

    #[test]
    fn sort_children_puts_dirs_first() {
        let mut node = TreeNode {
            name: String::from("n"),
            is_dir: true,
            children: Vec::new(),
        };
        node.push_child(TreeNode {
            name: String::from("z.txt"),
            is_dir: false,
            children: Vec::new(),
        });
        node.push_child(TreeNode {
            name: String::from("a"),
            is_dir: true,
            children: Vec::new(),
        });
        node.push_child(TreeNode {
            name: String::from("b.txt"),
            is_dir: false,
            children: Vec::new(),
        });
        node.push_child(TreeNode {
            name: String::from("c"),
            is_dir: true,
            children: Vec::new(),
        });
        node.sort_children();
        let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c", "b.txt", "z.txt"]);
    }

    #[test]
    fn child_path_joins_correctly() {
        let base = Path::new("/tmp/base");
        let p = child_path(base, "child");
        assert_eq!(p, Path::new("/tmp/base/child"));
    }

    #[test]
    fn read_tree_missing_path_errors() {
        let missing = Path::new("/definitely/not/a/real/path/for/tree-printer-directory");
        let result = read_tree(missing);
        assert!(result.is_err());
    }

    #[test]
    fn render_tree_on_missing_path_errors() {
        let missing = Path::new("/definitely/not/a/real/path/for/tree-printer-directory");
        assert!(render_tree(missing).is_err());
    }

    #[test]
    fn render_tree_with_on_missing_path_errors() {
        let missing = Path::new("/definitely/not/a/real/path/for/tree-printer-directory");
        let opts = TreeOptions::default();
        assert!(render_tree_with(missing, &opts).is_err());
    }
}
