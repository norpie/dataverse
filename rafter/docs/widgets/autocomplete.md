# Autocomplete

A text input with fuzzy-filtered suggestions.

## State Field

```rust
#[app]
struct MyApp {
    search: Autocomplete,
}
```

## Item Trait

For custom types, implement `AutocompleteItem`:

```rust
#[derive(Clone)]
struct City {
    code: String,
    name: String,
}

impl AutocompleteItem for City {
    fn autocomplete_id(&self) -> String {
        self.code.clone()
    }

    fn autocomplete_label(&self) -> String {
        self.name.clone()
    }
}
```

## Basic Usage

With strings:

```rust
fn page(&self) -> Node {
    let fruits = vec!["Apple", "Banana", "Cherry", "Date"];
    page! {
        autocomplete(bind: self.fruit, options: fruits)
    }
}
```

With custom types:

```rust
fn page(&self) -> Node {
    let cities = vec![
        City { code: "NYC".into(), name: "New York".into() },
        City { code: "LA".into(), name: "Los Angeles".into() },
    ];
    page! {
        autocomplete(bind: self.city, options: cities)
    }
}
```

## Initialization

```rust
async fn on_start(&self, _cx: &AppContext) {
    self.search.set_placeholder("Type to search...");
}
```

## Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `bind` | Autocomplete | The Autocomplete field to bind |
| `options` | Vec<T> | Available suggestions |
| `on_change` | handler | Called on input change |
| `on_select` | handler | Called when suggestion selected |

## Methods

| Method | Description |
|--------|-------------|
| `set_placeholder(s)` | Set placeholder text |
| `value()` | Get current input text |
| `set_value(s)` | Set input text |
| `clear()` | Clear input |

## Event Handlers

### on_change

Called when the input text changes:

```rust
#[handler]
async fn on_search_change(&self, _cx: &AppContext) {
    let query = self.search.value();
    // Update results based on query
}
```

### on_select

Called when a suggestion is selected:

```rust
#[handler]
async fn on_select(&self, cx: &AppContext) {
    let value = self.search.value();
    cx.toast(format!("Selected: {}", value));
}
```

## Fuzzy Matching

Autocomplete uses fuzzy matching to filter suggestions. Users can type partial matches:

- "ny" matches "New York"
- "la" matches "Los Angeles"
- "chgo" matches "Chicago"

## Keyboard Navigation

- Type: Filter suggestions
- Up/Down: Navigate suggestions
- Enter: Select highlighted suggestion
- Escape: Close dropdown
- Tab: Move to next widget

## Free-Form Input

Unlike Select, Autocomplete allows entering values not in the list:

```rust
#[handler]
async fn on_submit(&self, cx: &AppContext) {
    let value = self.search.value();
    // Value can be anything the user typed
}
```

## Complete Example

```rust
#[derive(Clone)]
struct Country {
    code: String,
    name: String,
}

impl AutocompleteItem for Country {
    fn autocomplete_id(&self) -> String { self.code.clone() }
    fn autocomplete_label(&self) -> String { self.name.clone() }
}

#[app]
struct AutocompleteDemo {
    fruit: Autocomplete,
    country: Autocomplete,
    last_event: String,
}

#[app_impl]
impl AutocompleteDemo {
    async fn on_start(&self, _cx: &AppContext) {
        self.fruit.set_placeholder("Type to search fruits...");
        self.country.set_placeholder("Search countries...");
        self.last_event.set("(none)".to_string());
    }

    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn on_fruit_change(&self, _cx: &AppContext) {
        let value = self.fruit.value();
        self.last_event.set(format!("Fruit input: \"{}\"", value));
    }

    #[handler]
    async fn on_fruit_select(&self, _cx: &AppContext) {
        let value = self.fruit.value();
        self.last_event.set(format!("Fruit selected: \"{}\"", value));
    }

    #[handler]
    async fn on_country_select(&self, _cx: &AppContext) {
        let value = self.country.value();
        self.last_event.set(format!("Country selected: \"{}\"", value));
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        let fruits = vec![
            "Apple", "Apricot", "Banana", "Blueberry", "Cherry",
            "Date", "Fig", "Grape", "Kiwi", "Lemon", "Mango",
            "Orange", "Peach", "Pear", "Plum", "Raspberry",
        ];

        let countries = vec![
            Country { code: "US".into(), name: "United States".into() },
            Country { code: "GB".into(), name: "United Kingdom".into() },
            Country { code: "CA".into(), name: "Canada".into() },
            Country { code: "AU".into(), name: "Australia".into() },
            Country { code: "DE".into(), name: "Germany".into() },
            Country { code: "FR".into(), name: "France".into() },
            Country { code: "JP".into(), name: "Japan".into() },
        ];

        let last_event = self.last_event.get();

        page! {
            column (padding: 2, gap: 2) {
                text (bold) { "Autocomplete Demo" }

                column (gap: 1) {
                    text { "Fruit:" }
                    autocomplete(
                        bind: self.fruit,
                        options: fruits,
                        on_change: on_fruit_change,
                        on_select: on_fruit_select
                    )
                }

                column (gap: 1) {
                    text { "Country:" }
                    autocomplete(
                        bind: self.country,
                        options: countries,
                        on_select: on_country_select
                    )
                }

                column (gap: 1) {
                    text (bold, fg: info) { "Last Event:" }
                    text (fg: muted) { last_event }
                }

                text (fg: muted) { "Tab to switch, type to filter, q to quit" }
            }
        }
    }
}
```
