# Views

Views define the visual structure of your app using a declarative, HTML-inspired syntax.

## Basic Syntax

```rust
fn view(&self) -> Node {
    view! {
        column {
            text { "Hello, world!" }
        }
    }
}
```

### With Styling

Styling is optional and uses parentheses:

```rust
view! {
    column (padding: 1, gap: 1) {
        text (bold, color: primary) { "Title" }
        text { "Plain text" }
        text (color: text_muted) { "Muted text" }
    }
}
```

### Boolean Attributes

Boolean styles can be written as shorthand:

```rust
// These are equivalent:
text (bold: true) { }
text (bold) { }

// Multiple booleans
text (bold, italic, underline) { }
```

## Primitives

### Layout Elements

```rust
// Vertical stack
column {
    text { "First" }
    text { "Second" }
}

// Horizontal stack
row {
    text { "Left" }
    text { "Right" }
}

// Z-index layering
stack {
    background_layer()
    foreground_layer()
}
```

### Content Elements

```rust
// Text display
text { "Hello" }
text { self.count.to_string() }

// Text input
input (on_change: handle_input, on_submit: handle_submit) { }

// Button
button (on_click: handle_click) { "Click me" }
```

### List Elements

```rust
// Virtualized list (for large datasets)
list (
    items: &self.records,
    item_height: 1,
    on_render: render_record,
)

// Table with columns
table (items: &self.records) {
    column (header: "Name", width: 30) { |r| r.name }
    column (header: "Status", width: 15) { |r| r.status }
    column (header: "Created", width: 20) { |r| r.created_at }
}

// Tree (collapsible nested items)
tree (items: &self.hierarchy, on_render: render_node)
```

### Other Elements

```rust
// Scrollable region
scroll {
    // content
}

// Selection elements
select (options: &self.options, selected: self.selected) { }
checkbox (checked: self.enabled, on_change: toggle) { "Enable feature" }
radio (options: &self.choices, selected: self.choice) { }

// Autocomplete input
autocomplete (
    suggestions: &self.suggestions,
    on_change: update_suggestions,
    on_select: select_suggestion,
) { }
```

## Control Flow

### Conditionals

```rust
view! {
    column {
        if self.loading {
            spinner { }
        } else {
            text { "Loaded!" }
        }
        
        // Optional content
        if let Some(error) = &self.error {
            text (color: error) { error }
        }
    }
}
```

### Loops

```rust
view! {
    column {
        for record in &self.records {
            row {
                text { record.name }
            }
        }
        
        // With index
        for (i, record) in self.records.iter().enumerate() {
            row (bg: if i == self.selected { surface } else { background }) {
                text { record.name }
            }
        }
    }
}
```

### Match

```rust
view! {
    column {
        match &self.records {
            Resource::Idle => text { "Press Enter to load" },
            Resource::Loading => spinner { },
            Resource::Progress(p) => progress_bar (value: p.current, max: p.total) { },
            Resource::Ready(data) => list (items: data) { ... },
            Resource::Error(e) => text (color: error) { e.to_string() },
        }
    }
}
```

## Components

Reusable view fragments are defined as components:

```rust
#[component]
fn record_row(record: &Record, selected: bool) -> Node {
    view! {
        row (bg: if selected { surface } else { background }) {
            text (width: 30) { record.name }
            text (width: 15, color: text_muted) { record.status }
        }
    }
}

// Usage
view! {
    column {
        for (i, record) in self.records.iter().enumerate() {
            record_row(record, i == self.selected)
        }
    }
}
```

### Components with Children

```rust
#[component]
fn card(title: &str, children: Children) -> Node {
    view! {
        column (border: rounded, padding: 1) {
            text (bold) { title }
            { children }
        }
    }
}

// Usage
view! {
    card("Details") {
        text { "Some content" }
        text { "More content" }
    }
}
```

### Optional Children

```rust
#[component]
fn card(title: &str, children: Option<Children>) -> Node {
    view! {
        column (border: rounded, padding: 1) {
            text (bold) { title }
            if let Some(c) = children {
                { c }
            }
        }
    }
}

// Both valid:
card("Empty Card")
card("With Content") {
    text { "Inside" }
}
```

## Event Handlers

Attach handlers to interactive elements:

```rust
view! {
    column {
        input (
            on_change: handle_input,
            on_submit: handle_submit,
        ) { }
        
        row (on_click: handle_row_click) {
            text { record.name }
        }
        
        button (on_click: save) { "Save" }
    }
}
```

Handlers receive event details:

```rust
#[handler]
async fn handle_row_click(&mut self, event: ClickEvent, cx: AppContext) {
    match event.kind {
        ClickKind::Primary => self.open_record(),
        ClickKind::Secondary => self.show_context_menu(cx).await,
    }
}

#[handler]
fn handle_input(&mut self, event: InputEvent) {
    self.filter = event.value;
}
```

## Virtualization

For large lists, Rafter automatically virtualizes rendering:

```rust
view! {
    list (
        items: &self.records,      // Could be thousands
        item_height: 1,            // Fixed height for calculation
        on_render: render_record,
    )
}

#[component]
fn render_record(record: &Record, index: usize, selected: bool) -> Node {
    view! {
        row (bg: if selected { surface } else { background }) {
            text { record.name }
        }
    }
}
```

The list component:
- Only renders visible items
- Calculates visible range from viewport
- Handles scroll offset
- Recycles nodes when scrolling

### Variable Height

```rust
list (
    items: &self.records,
    item_height: estimate_height,
)

fn estimate_height(record: &Record) -> u16 {
    if record.has_description { 3 } else { 1 }
}
```

### Infinite Scroll

```rust
list (
    items: &self.records,
    on_end_reached: load_more,
)

#[handler]
async fn load_more(&mut self, cx: AppContext) {
    let next_page = cx.api.fetch_page(self.page + 1).await;
    self.records.extend(next_page);
    self.page += 1;
}
```
