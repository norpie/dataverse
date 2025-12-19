# Async

Rafter has first-class async support built around safe state management.

## Sync vs Async Handlers

### Sync Handlers

Sync handlers get `&mut self` - exclusive mutable access:

```rust
#[handler]
fn select_next(&mut self) {
    self.selected += 1;
}
```

Use for: UI state updates, navigation, anything that doesn't need to wait.

### Async Handlers

Async handlers get `&self` - shared access only:

```rust
#[handler]
async fn load_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    let data = cx.api.get_records().await;
    self.records.set(Resource::Ready(data));
}
```

Use for: Network requests, file I/O, any operation that awaits.

## Why the Split?

The borrow checker prevents holding `&mut self` across await points - another handler couldn't run while waiting. The split ensures:

- Sync handlers run fast, have full access
- Async handlers can await, but only mutate designated fields
- No deadlocks or data races

## Async State

Only certain fields can be mutated from async handlers:

### Resource<T>

Resources are implicitly async state:

```rust
#[app]
struct MyApp {
    records: Resource<Vec<Record>>,  // Automatically async state
}

#[handler]
async fn load(&self, cx: AppContext) {
    self.records.set(Resource::Loading);  // OK
}
```

### Explicit Async State

For non-Resource types:

```rust
#[app]
struct MyApp {
    #[async_state]
    status: String,
}

#[handler]
async fn update(&self, cx: AppContext) {
    self.status.set("Working...".into());  // OK
}
```

### Summary

| State Type | Sync Handler | Async Handler |
|------------|--------------|---------------|
| Regular | Read + Write | Read only |
| `Resource<T>` | Read + Write | Read + Write |
| `#[async_state]` | Read + Write | Read + Write |

## Cancellation

### The Problem

What happens when a user triggers a load, then triggers it again?

```rust
// User presses "refresh"
// load_records starts, self.records = Loading
// User presses "refresh" again
// Another load_records starts
// First one finishes - overwrites with stale data?
```

### Handler Attributes

Control concurrent handler behavior:

```rust
#[handler(supersedes)]  // New call cancels previous
async fn load_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    let data = cx.api.get_records().await;
    self.records.set(Resource::Ready(data));
}

#[handler(queues)]  // Calls run sequentially
async fn save_record(&self, cx: AppContext) {
    // Saves won't overlap
}

#[handler(debounce = 300ms)]  // Wait for pause in calls
async fn search(&self, cx: AppContext) {
    // Only runs after 300ms of no new calls
}
```

### Manual Cancellation

For fine-grained control:

```rust
#[app]
struct MyApp {
    records: Resource<Vec<Record>>,
    #[state(skip)]
    load_token: CancellationToken,
}

#[handler]
async fn load_records(&self, cx: AppContext) {
    // Cancel any previous load
    self.load_token.cancel();
    let token = cx.cancellation_token();
    self.load_token = token.clone();
    
    self.records.set(Resource::Loading);
    
    let result = cx.api.get_records()
        .cancelable(&token)
        .await;
    
    match result {
        Ok(data) => self.records.set(Resource::Ready(data)),
        Err(Cancelled) => { /* Superseded, do nothing */ },
        Err(e) => self.records.set(Resource::Error(e)),
    }
}
```

## Progress Updates

Update progress without channels - just mutate state:

```rust
#[handler]
async fn load_all_records(&self, cx: AppContext) {
    self.records.set(Resource::Loading);
    
    let total = cx.api.get_count().await;
    let mut all = Vec::new();
    
    for page in 0..total_pages {
        let batch = cx.api.get_page(page).await;
        all.extend(batch);
        
        // Each update triggers a re-render
        self.records.set(Resource::Progress(ProgressState {
            current: all.len() as u64,
            total: Some(total),
            message: Some(format!("Loading... {}/{}", all.len(), total)),
        }));
    }
    
    self.records.set(Resource::Ready(all));
}
```

## Spawning Tasks

For fire-and-forget work:

```rust
#[handler]
async fn start_background_sync(&self, cx: AppContext) {
    cx.spawn(async move {
        loop {
            sync_data().await;
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
```

### Tracked Tasks

For tasks that report back:

```rust
#[handler]
async fn export(&self, cx: AppContext) {
    self.export_status.set(Resource::Loading);
    
    let result = cx.spawn_tracked(async move {
        heavy_export_work().await
    }).await;
    
    match result {
        Ok(path) => {
            self.export_status.set(Resource::Ready(path));
            cx.toast("Export complete");
        }
        Err(e) => self.export_status.set(Resource::Error(e)),
    }
}
```

## Pub/Sub

Apps communicate asynchronously via pub/sub:

### Publishing

```rust
#[handler]
async fn delete_record(&self, cx: AppContext) {
    cx.api.delete(self.selected.id).await;
    cx.publish(RecordDeleted { id: self.selected.id });
}
```

### Subscribing

Handlers that accept an event type are automatically subscribed:

```rust
// This handler runs when any app publishes RecordDeleted
#[handler]
async fn on_record_deleted(&self, event: RecordDeleted, cx: AppContext) {
    self.records.retain(|r| r.id != event.id);
    cx.toast(format!("Record {} deleted", event.id));
}
```

### Cross-App Communication

```rust
// In ExplorerApp
cx.publish(QueueOperation {
    kind: OperationKind::Export,
    record_id: self.selected.id,
});

// In QueueApp (running in background)
#[handler]
async fn on_queue_operation(&self, op: QueueOperation, cx: AppContext) {
    self.queue.push(op);
    self.process_queue().await;
}
```
