//! Family-correct completion callbacks for the stream's existing admission owner.
use crate::{Execution, Local, Threaded};

/// Completion callback representation without changing stream storage or observation.
pub trait StreamExecution<T, R>: Execution {
    /// Synchronous terminal-event recognition.
    type Classifier: Fn(&T) -> bool + ?Sized;
    /// Synchronous extraction of the retained final result.
    type Extractor: Fn(&T) -> R + ?Sized;
}

/// Select callback auto-traits while preserving each family's owning pointer.
macro_rules! callbacks {
    ($family:ty, [$($bound:tt)*]) => {
        impl<T, R> StreamExecution<T, R> for $family {
            type Classifier = dyn Fn(&T) -> bool $($bound)*;
            type Extractor = dyn Fn(&T) -> R $($bound)*;
        }
    };
}
callbacks!(Local, []);
#[cfg(not(target_arch = "wasm32"))]
callbacks!(Threaded, [+ Send + Sync]);
#[cfg(target_arch = "wasm32")]
callbacks!(Threaded, []);
