# Table

A data table with columns, sorting, and row selection.

## State Field

```rust
#[app]
struct MyApp {
    users: Table<User>,
}
```

## Row Trait

Implement `TableRow` for your data type:

```rust
#[derive(Clone)]
struct User {
    id: String,
    name: String,
    email: String,
    age: u32,
}

impl TableRow for User {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.email.clone(),
            self.age.to_string(),
        ]
    }
}
```

## Basic Usage

```rust
fn page(&self) -> Node {
    let columns = vec![
        Column::new("Name").width(20),
        Column::new("Email").width(30),
        Column::new("Age").width(5).alignment(Alignment::Right),
    ];

    page! {
        table(bind: self.users, columns: columns, height: fill)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    let users = vec![
        User { id: "1".into(), name: "Alice".into(), email: "alice@example.com".into(), age: 30 },
        User { id: "2".into(), name: "Bob".into(), email: "bob@example.com".into(), age: 25 },
    ];
    self.users.set_items(users);
    self.users.set_selection_mode(SelectionMode::Single);
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Table<T> | The Table field to bind |
| `columns` | Vec<Column> | Column definitions |
| `on_activate` | handler | Called on Enter key |
| `on_selection_change` | handler | Called when selection changes |
| `on_cursor_move` | handler | Called when cursor moves |
| `on_sort` | handler | Called when column header clicked |
| `height` | number/fill | Table height |
| `width` | number/fill | Table width |

## Column Definition

```rust
Column::new("Name")          // Title only
    .width(20)               // Fixed width
    .alignment(Alignment::Left)   // Left/Center/Right
    .sortable(true)          // Enable sorting
```

### Alignment Options

- `Alignment::Left` (default)
- `Alignment::Center`
- `Alignment::Right`

## Methods

| Method | Description |
|--------|-------------|
| `set_items(vec)` | Set table rows |
| `items()` | Get all items |
| `len()` | Row count |
| `set_selection_mode(mode)` | Single/Multiple/None |
| `set_cursor(index)` | Move cursor to row |
| `cursor_index()` | Get cursor index |
| `selected_ids()` | Get selected row IDs |
| `selected_items()` | Get selected items |
| `sort_by(column, ascending)` | Sort by column |

## Event Handlers

### on_sort

Called when a column header is clicked:

```rust
#[handler]
async fn on_sort(&self, cx: &AppContext) {
    if let Some((column, ascending)) = cx.sorted_column() {
        let mut items = self.users.items();
        match column {
            0 => items.sort_by(|a, b| a.name.cmp(&b.name)),
            1 => items.sort_by(|a, b| a.email.cmp(&b.email)),
            2 => items.sort_by(|a, b| a.age.cmp(&b.age)),
            _ => {}
        }
        if !ascending {
            items.reverse();
        }
        self.users.set_items(items);
    }
}
```

### on_activate

Called when Enter is pressed:

```rust
#[handler]
async fn on_row_activate(&self, cx: &AppContext) {
    if let Some(id) = cx.activated_id() {
        cx.toast(format!("Selected user: {}", id));
    }
}
```

## Keyboard Navigation

- Up/Down or j/k: Move cursor between rows
- Left/Right: Scroll horizontally (if needed)
- Enter: Activate row
- Space: Toggle selection (Multiple mode)

## Complete Example

```rust
#[derive(Clone)]
struct Product {
    id: String,
    name: String,
    price: f64,
    stock: u32,
}

impl TableRow for Product {
    fn id(&self) -> String { self.id.clone() }

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            format!("${:.2}", self.price),
            self.stock.to_string(),
        ]
    }
}

#[app]
struct ProductTable {
    products: Table<Product>,
}

#[app_impl]
impl ProductTable {
    async fn on_start(&self, _cx: &AppContext) {
        self.products.set_items(vec![
            Product { id: "1".into(), name: "Widget".into(), price: 9.99, stock: 100 },
            Product { id: "2".into(), name: "Gadget".into(), price: 24.99, stock: 50 },
            Product { id: "3".into(), name: "Gizmo".into(), price: 14.99, stock: 75 },
        ]);
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_activate(&self, cx: &AppContext) {
        if let Some(id) = cx.activated_id() {
            if let Some(product) = self.products.items().iter().find(|p| p.id == id) {
                cx.toast(format!("View: {} - ${:.2}", product.name, product.price));
            }
        }
    }

    #[handler]
    async fn on_sort(&self, cx: &AppContext) {
        if let Some((col, asc)) = cx.sorted_column() {
            let mut items = self.products.items();
            match col {
                0 => items.sort_by(|a, b| a.name.cmp(&b.name)),
                1 => items.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap()),
                2 => items.sort_by(|a, b| a.stock.cmp(&b.stock)),
                _ => {}
            }
            if !asc { items.reverse(); }
            self.products.set_items(items);
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let columns = vec![
            Column::new("Product").width(20).sortable(true),
            Column::new("Price").width(10).alignment(Alignment::Right).sortable(true),
            Column::new("Stock").width(8).alignment(Alignment::Right).sortable(true),
        ];

        page! {
            column (padding: 1, gap: 1) {
                text (bold) { "Products" }
                table(
                    bind: self.products,
                    columns: columns,
                    on_activate: on_activate,
                    on_sort: on_sort,
                    height: fill
                )
                text (fg: muted) { "Click column to sort, Enter to view, q to quit" }
            }
        }
    }
}
```
