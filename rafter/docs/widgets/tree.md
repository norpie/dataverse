# Tree

A hierarchical tree view with expand/collapse support.

## State Field

```rust
#[app]
struct MyApp {
    files: Tree<FileNode>,
}
```

## Item Trait

Implement `TreeItem` for your data type:

```rust
#[derive(Clone)]
struct FileNode {
    path: String,
    name: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

impl TreeItem for FileNode {
    fn id(&self) -> String {
        self.path.clone()
    }

    fn children(&self) -> Vec<Self> {
        self.children.clone()
    }

    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
        // Use the default helper for common styling
        Self::render_default(&self.name, focused, selected, depth, self.is_dir, expanded)
    }
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    page! {
        tree(bind: self.files, height: fill, width: fill)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    let tree = vec![
        FileNode::dir("/src", "src", vec![
            FileNode::file("/src/main.rs", "main.rs"),
            FileNode::file("/src/lib.rs", "lib.rs"),
        ]),
        FileNode::file("/Cargo.toml", "Cargo.toml"),
    ];
    self.files.set_items(tree);
    self.files.set_selection_mode(SelectionMode::Single);
    self.files.expand("/src");  // Expand by ID
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Tree<T> | The Tree field to bind |
| `on_activate` | handler | Called on Enter key |
| `on_expand` | handler | Called when node expands |
| `on_collapse` | handler | Called when node collapses |
| `on_selection_change` | handler | Called when selection changes |
| `on_cursor_move` | handler | Called when cursor moves |
| `height` | number/fill | Tree height |
| `width` | number/fill | Tree width |

## Methods

| Method | Description |
|--------|-------------|
| `set_items(vec)` | Set root items |
| `find(id)` | Find node by ID |
| `expand(id)` | Expand a node |
| `collapse(id)` | Collapse a node |
| `toggle(id)` | Toggle expand/collapse |
| `is_expanded(id)` | Check if expanded |
| `expand_all()` | Expand all nodes |
| `collapse_all()` | Collapse all nodes |
| `set_selection_mode(mode)` | Single/Multiple/None |
| `selected_ids()` | Get selected IDs |
| `cursor_id()` | Get cursor node ID |
| `visible_len()` | Count visible nodes |

## Event Handlers

### on_activate

Called when Enter is pressed:

```rust
#[handler]
async fn on_activate(&self, cx: &AppContext) {
    if let Some(id) = cx.activated_id() {
        if let Some(node) = self.files.find(&id) {
            if node.is_dir {
                self.files.toggle(&id);
            } else {
                cx.toast(format!("Opening: {}", node.name));
            }
        }
    }
}
```

### on_expand / on_collapse

Called when nodes expand or collapse:

```rust
#[handler]
async fn on_expand(&self, cx: &AppContext) {
    if let Some(id) = cx.expanded_id() {
        cx.toast(format!("Expanded: {}", id));
    }
}

#[handler]
async fn on_collapse(&self, cx: &AppContext) {
    if let Some(id) = cx.collapsed_id() {
        cx.toast(format!("Collapsed: {}", id));
    }
}
```

## Custom Rendering

Override `render` for custom appearance:

```rust
impl TreeItem for FileNode {
    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
        let indent = "  ".repeat(depth as usize);
        let icon = if self.is_dir {
            if expanded { "v " } else { "> " }
        } else {
            "  "
        };
        let marker = if selected { "*" } else { " " };

        page! {
            row {
                text { format!("{}{}{}{}", indent, marker, icon, self.name) }
            }
        }
    }
}
```

## Keyboard Navigation

- Up/Down or j/k: Move cursor
- Left or h: Collapse node or go to parent
- Right or l: Expand node or go to first child
- Enter: Activate (open file or toggle folder)
- Space: Toggle selection

## Complete Example

```rust
#[derive(Clone)]
struct Category {
    id: String,
    name: String,
    items: Vec<Category>,
}

impl Category {
    fn new(id: &str, name: &str, items: Vec<Category>) -> Self {
        Self { id: id.into(), name: name.into(), items }
    }
}

impl TreeItem for Category {
    fn id(&self) -> String { self.id.clone() }
    fn children(&self) -> Vec<Self> { self.items.clone() }

    fn render(&self, focused: bool, selected: bool, depth: u16, expanded: bool) -> Node {
        let has_children = !self.items.is_empty();
        Self::render_default(&self.name, focused, selected, depth, has_children, expanded)
    }
}

#[app]
struct TreeDemo {
    tree: Tree<Category>,
}

#[app_impl]
impl TreeDemo {
    async fn on_start(&self, _cx: &AppContext) {
        let data = vec![
            Category::new("electronics", "Electronics", vec![
                Category::new("phones", "Phones", vec![
                    Category::new("iphone", "iPhone", vec![]),
                    Category::new("android", "Android", vec![]),
                ]),
                Category::new("laptops", "Laptops", vec![]),
            ]),
            Category::new("books", "Books", vec![
                Category::new("fiction", "Fiction", vec![]),
                Category::new("nonfiction", "Non-Fiction", vec![]),
            ]),
        ];
        self.tree.set_items(data);
        self.tree.expand("electronics");
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "e" => expand_all,
            "c" => collapse_all,
            "q" => quit,
        }
    }

    #[handler]
    async fn expand_all(&self, _cx: &AppContext) {
        self.tree.expand_all();
    }

    #[handler]
    async fn collapse_all(&self, _cx: &AppContext) {
        self.tree.collapse_all();
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            column (padding: 1, gap: 1) {
                text (bold) { "Tree Demo" }
                tree(bind: self.tree, height: fill, width: fill)
                text (fg: muted) { "Arrows to navigate, e expand all, c collapse all, q quit" }
            }
        }
    }
}
```
