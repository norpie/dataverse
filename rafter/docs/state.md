# State Management

Rafter uses a Svelte-inspired reactivity model with automatic change tracking.

## Basic State

State is defined as fields on your app struct:

```rust
#[app]
struct ExplorerApp {
    records: Vec<Record>,
    selected: usize,
    filter: String,
}
```

The `#[app]` macro automatically wraps fields in `State<T>` for change tracking:

```rust
// What you write:
#[app]
struct ExplorerApp {
    selected: usize,
}

// What the macro generates:
struct ExplorerApp {
    selected: State<usize>,
}
```

## Mutating State

In sync handlers, just mutate directly:

```rust
#[handler]
fn select_next(&mut self) {
    self.selected += 1;  // DerefMut triggers change tracking
}

#[handler]
fn set_filter(&mut self, value: String) {
    self.filter = value;
}
```

The framework detects changes via `DerefMut` and triggers re-renders automatically.

## Skipping Tracking

Some fields shouldn't trigger re-renders (caches, internal state):

```rust
#[app]
struct ExplorerApp {
    records: Vec<Record>,
    selected: usize,
    
    #[state(skip)]  // Not tracked, not reactive
    cache: HashMap<String, CachedData>,
}
```

## Resources

`Resource<T>` is a special type for async-loaded data:

```rust
enum Resource<T> {
    Idle,                      // Not started
    Loading,                   // Indeterminate spinner
    Progress(ProgressState),   // Determinate progress bar
    Ready(T),                  // Data loaded
    Error(ResourceError),      // Failed
}

struct ProgressState {
    current: u64,
    total: Option<u64>,
    message: Option<String>,
}
```

### Using Resources

```rust
#[app]
struct ExplorerApp {
    records: Resource<Vec<Record>>,  // Implicitly async state
}

fn view(&self) -> Node {
    view! {
        column {
            match &self.records {
                Resource::Idle => text { "Press Enter to load" },
                Resource::Loading => spinner { },
                Resource::Progress(p) => {
                    progress_bar (value: p.current, max: p.total) {
                        p.message
                    }
                },
                Resource::Ready(records) => {
                    list (items: records, on_render: render_record)
                },
                Resource::Error(e) => {
                    text (color: error) { e.to_string() }
                },
            }
        }
    }
}
```

### Updating Resources

Resources are implicitly async state - they can be updated from async handlers:

```rust
#[handler]
async fn load_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    match cx.api.get_records().await {
        Ok(data) => self.records.set(Resource::Ready(data)),
        Err(e) => self.records.set(Resource::Error(e.into())),
    }
}
```

### Progress Updates

```rust
#[handler]
async fn load_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    let total = cx.api.get_count().await;
    let mut all = Vec::new();
    
    for page in 0..total_pages {
        let batch = cx.api.get_page(page).await;
        all.extend(batch);
        
        // Progress updates trigger re-renders
        self.records.set(Resource::Progress(ProgressState {
            current: all.len() as u64,
            total: Some(total),
            message: Some(format!("Loading... {}/{}", all.len(), total)),
        }));
    }
    
    self.records.set(Resource::Ready(all));
}
```

## Sync vs Async State

| Type | Sync Handler (`&mut self`) | Async Handler (`&self`) |
|------|---------------------------|-------------------------|
| Regular fields | Read + Write | Read only |
| `Resource<T>` | Read + Write | Read + Write |
| `#[async_state]` | Read + Write | Read + Write |

### Explicit Async State

For non-Resource types that need async mutation:

```rust
#[app]
struct MyApp {
    #[async_state]
    status: String,  // Can be updated from async handlers
}

#[handler]
async fn update_status(&self, cx: AppContext) {
    self.status.set("Working...".into());
    do_work().await;
    self.status.set("Done".into());
}
```

This should be rare - most async work uses `Resource<T>`.

## How It Works

Under the hood, `State<T>` tracks changes:

```rust
struct State<T> {
    value: T,
    dirty: bool,
}

impl<T> Deref for State<T> {
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for State<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.dirty = true;  // Mark changed
        &mut self.value
    }
}
```

After a handler completes, the framework checks for dirty state and re-renders if needed.
