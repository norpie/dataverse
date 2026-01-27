//! Simple loading modal for executing a single async operation.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use tuidom::Element;

use crate::widgets::BrailleSpinner;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Simple loading modal for a single operation.
///
/// Shows a loading message with a spinner while executing an async operation.
/// Returns the result directly.
///
/// # Example
///
/// ```ignore
/// let attrs = gx.modal(LoadingModal::new(
///     "Loading attributes",
///     client.metadata().attributes(entity)
/// )).await;
/// ```
#[modal]
pub struct LoadingModal<T> {
    #[state(skip)]
    message: String,

    #[state(skip)]
    operation: Arc<Mutex<Option<BoxFuture<'static, T>>>>,
}

impl<T: Send + Sync + 'static> LoadingModal<T> {
    /// Create a new loading modal.
    pub fn new<F>(message: impl Into<String>, operation: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        Self {
            message: message.into(),
            operation: Arc::new(Mutex::new(Some(Box::pin(operation)))),
            ..Default::default()
        }
    }
}

#[modal_impl(Result = Option<T>)]
#[rustfmt::skip]
impl<T: Send + Sync + 'static> LoadingModal::<T> {
    fn default_result(&self) -> Option<T> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<T>>) {
        let operation = self.operation.lock().unwrap().take();

        let Some(op) = operation else {
            mx.close(None);
            return;
        };

        let result = op.await;
        mx.close(Some(result));
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                row (gap: 1) {
                    BrailleSpinner {}
                    text (content: self.message.clone()) style (fg: primary)
                }
            }
        }
    }
}
