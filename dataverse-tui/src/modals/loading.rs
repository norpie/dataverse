//! Simple loading modal for executing a single async operation.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use tuidom::Element;

use crate::widgets::Spinner;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type DefaultFn<T> = Arc<dyn Fn() -> T + Send + Sync>;

/// Simple loading modal for a single operation.
///
/// Shows a loading message with a spinner while executing an async operation.
/// Returns the result directly.
///
/// # Example
///
/// ```ignore
/// // For types implementing Default (e.g., Option<T>):
/// let data = gx.modal(LoadingModal::run(
///     "Loading data",
///     fetch_data()
/// )).await;
///
/// // For Result types, provide a shutdown fallback closure:
/// let result = gx.modal(LoadingModal::run_with_default(
///     "Loading attributes",
///     || Err(Error::Cancelled),
///     client.metadata().attributes(entity)
/// )).await;
/// ```
#[modal]
pub struct LoadingModal<T> {
    #[state(skip)]
    message: String,

    #[state(skip)]
    shutdown_default_fn: DefaultFn<T>,

    #[state(skip)]
    operation: Arc<Mutex<Option<BoxFuture<'static, T>>>>,
}

impl<T: Send + Sync + 'static> LoadingModal<T> {
    /// Run an async operation with a loading modal.
    ///
    /// Uses `T::default()` as the fallback value if the app shuts down
    /// while the operation is in progress.
    pub fn run<F>(message: impl Into<String>, operation: F) -> Self
    where
        T: Default,
        F: Future<Output = T> + Send + 'static,
    {
        Self::run_with_default(message, T::default, operation)
    }

    /// Run an async operation with a loading modal and explicit shutdown fallback.
    ///
    /// The closure is called to produce a fallback value if the app shuts down
    /// while the operation is in progress.
    pub fn run_with_default<F, D>(
        message: impl Into<String>,
        shutdown_default: D,
        operation: F,
    ) -> Self
    where
        F: Future<Output = T> + Send + 'static,
        D: Fn() -> T + Send + Sync + 'static,
    {
        Self::new(
            message.into(),
            Arc::new(shutdown_default),
            Arc::new(Mutex::new(Some(Box::pin(operation)))),
        )
    }
}

#[modal_impl(Result = T)]
#[rustfmt::skip]
impl<T: Send + Sync + 'static> LoadingModal::<T> {
    fn default_result(&self) -> T {
        (self.shutdown_default_fn)()
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<T>) {
        let operation = self.operation.lock().unwrap().take();

        let Some(op) = operation else {
            // Operation already taken - shouldn't happen, but use fallback
            mx.close((self.shutdown_default_fn)());
            return;
        };

        let result = op.await;
        mx.close(result);
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: self.message.clone()) style (fg: primary)
                spinner (id: "loading-spinner")
            }
        }
    }
}
