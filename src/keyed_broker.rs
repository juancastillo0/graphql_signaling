use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::task::{Context, Poll};
use futures::{Stream, StreamExt};
use once_cell::sync::Lazy;
use slab::Slab;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Mutex;

static SUBSCRIBERS: Lazy<
    Mutex<HashMap<TypeId, Mutex<HashMap<String, Box</*Senders<TypeId>*/ dyn Any + Send>>>>>,
> = Lazy::new(Default::default);

struct Senders<T>(Slab<UnboundedSender<T>>);

struct BrokerStream<T: Sync + Send + Clone + 'static>(
    String,
    usize,
    UnboundedReceiver<T>,
    Option<Box<dyn FnOnce() + Send>>,
);

fn with_senders<T, F, R>(key: String, f: F) -> R
where
    T: Sync + Send + Clone + 'static,
    F: FnOnce(&mut Senders<T>) -> R,
{
    let mut map = SUBSCRIBERS.lock().unwrap();
    let mut senders_map = map
        .entry(TypeId::of::<Senders<T>>())
        .or_default()
        .lock()
        .unwrap();

    let senders = senders_map
        .entry(key)
        .or_insert_with(|| Box::new(Senders::<T>(Default::default())));

    f(senders.downcast_mut::<Senders<T>>().unwrap())
}

impl<T: Sync + Send + Clone + 'static> Drop for BrokerStream<T> {
    fn drop(&mut self) {
        with_senders::<T, _, _>(self.0.clone(), |senders| senders.0.remove(self.1));
        if let Some(f) = self.3.take() {
            f();
        }
    }
}

impl<T: Sync + Send + Clone + 'static> Stream for BrokerStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.2.poll_next_unpin(cx)
    }
}

/// A simple broker based on memory
pub struct KeyedBroker<T>(PhantomData<T>);

impl<T: Sync + Send + Clone + 'static> KeyedBroker<T> {
    /// Publish a message that all subscription streams can receive.
    pub fn publish(key: String, msg: T) -> usize {
        with_senders::<T, _, _>(key, |senders| {
            for (_, sender) in senders.0.iter_mut() {
                sender.start_send(msg.clone()).ok();
            }
            senders.0.len()
        })
    }

    /// Subscribe to the message of the specified type and returns a `Stream`.
    pub fn subscribe(
        key: String,
        on_drop: Option<Box<dyn FnOnce() + Send>>,
    ) -> impl Stream<Item = T> {
        with_senders::<T, _, _>(key.clone(), |senders| {
            let (tx, rx) = mpsc::unbounded();
            let id = senders.0.insert(tx);
            BrokerStream(key, id, rx, on_drop)
        })
    }

    pub fn subscriber_count(key: String) -> usize {
        with_senders::<T, _, _>(key, |senders| senders.0.len())
    }
}
