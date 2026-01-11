//! Test helpers for the transport module.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::{ConnectionHandler, ConnectionStream};

pub(crate) struct CountingHandler {
    count: Arc<AtomicUsize>,
}

impl CountingHandler {
    pub(crate) fn new() -> (Arc<AtomicUsize>, Arc<Self>) {
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(Self {
            count: Arc::clone(&count),
        });
        (count, handler)
    }
}

impl ConnectionHandler for CountingHandler {
    fn handle(&self, _stream: ConnectionStream) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}
