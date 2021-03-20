use crate::keyed_broker::KeyedBroker;
use async_graphql::{Enum, Object, Result, ID};
use dashmap::{DashMap, DashSet};
// use futures::task::{Context, Poll};
use futures::future::{join_all, FutureExt};
use futures::{Stream, StreamExt};
use pin_project::{pin_project, pinned_drop};
use std::{
    collections::HashSet,
    task::{Context, Poll},
};
use std::{pin::Pin, sync::Arc};
use tokio::sync::{broadcast, watch, RwLock};
use tokio_stream::wrappers::{BroadcastStream, WatchStream};

use super::ROOMS;

#[derive(Default)]
pub struct RoomsStorage {
    pub rooms: DashMap<String, Observable<Room>>,
    pub users: DashMap<String, ObservableHistory<Signal>>,
}

impl RoomsStorage {
    pub async fn subscribe_to_room<O: From<Arc<Room>>>(
        &self,
        room_id: String,
        user_id: String,
    ) -> impl Stream<Item = O> {
        let room = self.get_or_insert_room(room_id.clone()).await;

        room.edit(|room| room.users.insert(user_id.clone()))
            .await
            .unwrap();
        // KeyedBroker::subscribe(
        //     "key".to_string(),
        //     Some(Box::new(|| {
        //         println!("dd");
        //     })),
        // );

        // {
        //     let mut users = room.value.write().await;
        //     dbg!(&users);
        //     if users.insert(user_id) {
        //         room.sender.send(Arc::new(users.clone())).unwrap();
        //     }
        // }
        room.clone().subscribe(Some(Box::new(|| {
            actix_rt::spawn(async move {
                let mut remove_room = false;
                room.edit(|room| {
                    if room.users.remove(&user_id) {
                        remove_room = room.users.len() == 0;
                        true
                    } else {
                        false
                    }
                })
                .await
                .unwrap();
                if remove_room {
                    ROOMS.rooms.remove(&room_id);
                }
            });
        })))
    }

    pub fn subscribe_to_signals<O: From<Arc<Signal>>>(
        &self,
        user_id: String,
    ) -> impl Stream<Item = O> {
        self.users
            .entry(user_id.clone())
            .or_insert_with(|| ObservableHistory::new(user_id.clone()))
            .subscribe(Some(Box::new(|| {
                actix_rt::spawn(async move {
                    let to_remove = Arc::new(DashSet::<String>::new());
                    let user_id = Arc::new(user_id);
                    join_all(
                        ROOMS.rooms.iter().map(|v| {
                            let user_id = user_id.clone();
                            let to_remove = to_remove.clone();
                            async move {
                                let room_key = v.key.clone();
                                v.edit(move |room| {
                                    if room.users.remove(user_id.as_ref()) {
                                        if room.users.len() == 0 {
                                            to_remove.insert(room_key);
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                })
                                .await
                                .unwrap();
                            }
                        }), // .collect::<dyn Future<Output=Result<bool, watch::error::SendError<Arc<Room>>>>>()
                    )
                    .await;

                    ROOMS.rooms.retain(|key, _room| !to_remove.contains(key));

                    ROOMS
                        .users
                        .remove_if(user_id.as_ref(), |_k, v| v.subscriber_count() == 0);
                });
            })))
    }

    async fn get_or_insert_room(&self, room_id: String) -> Observable<Room> {
        let room = self
            .rooms
            .entry(room_id.clone())
            .or_insert_with(|| Observable::new(room_id));
        room.clone()
    }

    // async fn subscribe_to_room(
    //     &self,
    //     user_id: String,
    //     payload: String
    // ) -> bool {
    //     let users = self.users.read().await;
    //     if let Some(user) = users.get(user_id) {

    //     } else {
    //         false
    //     }
    // }
}

#[derive(Clone)]
pub struct Observable<T> {
    key: String,
    sender: Arc<watch::Sender<Arc<T>>>,
    receiver: watch::Receiver<Arc<T>>,
    value: Arc<RwLock<T>>,
}

impl<T: Clone + Default + Send + Sync + 'static> Observable<T> {
    pub async fn edit(
        &self,
        mapper: impl FnOnce(&mut T) -> bool,
    ) -> Result<bool, watch::error::SendError<Arc<T>>> {
        let mut v = self.value.write().await;
        if mapper(&mut v) {
            self.send(v.clone()).map(|_| true)
        } else {
            Ok(false)
        }
    }

    pub fn send(&self, value: T) -> Result<(), watch::error::SendError<Arc<T>>> {
        self.sender.send(Arc::new(value))
        // KeyedBroker::<Arc<T>>::publish(self.key.clone(), Arc::new(value));
        // Ok(())
    }

    pub fn subscribe<O: From<Arc<T>>>(
        self,
        on_drop: Option<Box<dyn FnOnce() + Send>>,
    ) -> impl Stream<Item = O> {
        DropStream::new(WatchStream::new(self.receiver), || {
            if let Some(f) = on_drop {
                f();
            }
        })
        .map(|v| O::from(v))
        // watch_to_steam(self.receiver)
        // KeyedBroker::<Arc<T>>::subscribe(self.key, on_drop).map(|v| O::from(v))
    }

    fn new(key: String) -> Self {
        let value: T = Default::default();
        let (sender, receiver) = watch::channel(Arc::new(value.clone()));
        Self {
            key,
            sender: Arc::new(sender),
            receiver,
            value: Arc::new(RwLock::new(value)),
        }
    }
}

// impl<T: Default> std::default::Default for Observable<T> {

// }

// impl<T> Clone for Observable<T> {
//     fn clone(&self) -> Self {
//         Self {
//             sender: self.sender.clone(),
//             receiver: self.receiver.clone(),
//             value: self.value.clone(),
//         }
//     }
// }

pub struct ObservableHistory<T> {
    key: String,
    sender: broadcast::Sender<Arc<T>>,
}

impl<T: Send + Sync + 'static> ObservableHistory<T> {
    pub fn send(&self, value: T) -> Result<usize, broadcast::error::SendError<Arc<T>>> {
        self.sender.send(Arc::new(value))
        // Ok(KeyedBroker::<Arc<T>>::publish(
        //     self.key.clone(),
        //     Arc::new(value),
        // ))
    }

    pub fn subscribe<O: From<Arc<T>>>(
        &self,
        on_drop: Option<Box<dyn FnOnce() + Send>>,
    ) -> impl Stream<Item = O> {
        DropStream::new(BroadcastStream::new(self.sender.subscribe()), || {
            if let Some(f) = on_drop {
                f();
            }
        })
        .filter_map(|e| async { e.ok() })
        .map(|v| O::from(v))
        // KeyedBroker::<Arc<T>>::subscribe(self.key.clone(), on_drop).map(|v| O::from(v))
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
        // KeyedBroker::<Arc<T>>::subscriber_count(self.key.clone())
    }

    fn new(key: String) -> Self {
        let (sender, _) = broadcast::channel(25);
        Self { sender, key }
    }
}

// impl<T> std::default::Default for ObservableHistory<T> {

// }

// impl<T> Clone for ObservableHistory<T> {
//     fn clone(&self) -> Self {
//         Self {
//             sender: self.sender.clone(),
//             receiver: self.sender.subscribe(),
//         }
//     }
// }

fn broadcast_to_steam<O: From<Arc<T>>, T>(
    mut receiver: broadcast::Receiver<Arc<T>>,
) -> impl Stream<Item = O> {
    (async_stream::stream! {
        while let Some(value) = receiver.recv().await.ok() {
            yield value.clone();
        }
    })
    .map(|v| v.into())
}

fn watch_to_steam<O: From<Arc<T>>, T>(
    mut receiver: watch::Receiver<Arc<T>>,
) -> impl Stream<Item = O> {
    (async_stream::stream! {
        while receiver.changed().await.is_ok() {
            let _value = (*receiver.borrow()).clone();
            yield _value;
        }
    })
    .map(|v| v.into())
}

// struct DropStream<S: Stream>(S, Option<Box<dyn FnOnce() + Send>>);

// impl<S: Stream, F: FnOnce()> Drop for DropStream<S, F> {
//     fn drop(&mut self) {
//         if let Some(f) = self.1.take() {
//             f();
//         };
//     }
// }

#[pin_project(PinnedDrop)]
pub struct DropStream<St, F: FnOnce()> {
    #[pin]
    stream: St,
    f: Option<F>,
}

impl<St, F: FnOnce()> DropStream<St, F> {
    pub(crate) fn new(stream: St, f: F) -> Self {
        Self { stream, f: Some(f) }
    }
}

#[pinned_drop]
impl<S, F: FnOnce()> PinnedDrop for DropStream<S, F> {
    fn drop(self: Pin<&mut Self>) {
        let f: &mut Option<F> = self.project().f;
        if let Some(f) = f.take() {
            f();
        }
    }
}

impl<St, F: FnOnce()> Stream for DropStream<St, F>
where
    St: Stream,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let stream: Pin<&mut St> = self.project().stream;
        stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<St, F: FnOnce()> futures::stream::FusedStream for DropStream<St, F>
where
    St: futures::stream::FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

#[derive(Default, Clone, Debug)]
pub struct Room {
    pub users: HashSet<String>,
}

pub struct RoomObj(Arc<Room>);

#[Object(name = "Room")]
impl RoomObj {
    async fn users(&self) -> &HashSet<String> {
        &self.0.users
    }
}

impl From<Arc<Room>> for RoomObj {
    fn from(v: Arc<Room>) -> Self {
        Self(v)
    }
}

#[derive(Clone, Debug)]
pub struct Signal {
    pub payload: String,
    pub peer_id: String,
}

pub struct SignalObj(Arc<Signal>);

#[Object(name = "Signal")]
impl SignalObj {
    async fn payload(&self) -> &str {
        &self.0.payload
    }

    async fn peer_id(&self) -> &str {
        &self.0.peer_id
    }
}

impl From<Arc<Signal>> for SignalObj {
    fn from(v: Arc<Signal>) -> Self {
        Self(v)
    }
}

//
//

#[derive(Enum, Eq, PartialEq, Copy, Clone)]
enum RoomChangeType {
    Joined,
    Left,
}

#[derive(Clone)]
struct RoomChanged {
    change_type: RoomChangeType,
    user_id: ID,
    room_id: ID,
}
