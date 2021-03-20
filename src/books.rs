use crate::auth::SessionId;
use crate::simple_broker::SimpleBroker;

use async_graphql::*;
use async_graphql::{Context, Enum, Object, Result, Subscription, ID};

use futures::lock::Mutex;
use futures::{Stream, StreamExt};

use slab::Slab;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, serde::Deserialize)]
pub struct Book {
    pub id: ID,
    pub name: String,
    pub author: String,
}

#[Object]
impl Book {
    async fn id(&self) -> &str {
        &self.id
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn author(&self) -> &str {
        &self.author
    }
}

pub type Storage = Arc<Mutex<Slab<Book>>>;

#[derive(Default)]
pub struct BookQueries;

#[Object]
impl BookQueries {
    async fn books(&self, ctx: &Context<'_>) -> Vec<Book> {
        let books = ctx.data_unchecked::<Storage>().lock().await;
        books.iter().map(|(_, book)| book).cloned().collect()
    }

    #[tracing::instrument(skip(ctx, self))]
    async fn token<'a>(&self, ctx: &'a Context<'_>) -> Option<&'a str> {
        ctx.data_opt::<SessionId>().map(|token| token.value())
    }
}

pub struct BooksMutations;

#[Object]
impl BooksMutations {
    async fn create_book(&self, ctx: &Context<'_>, name: String, author: String) -> ID {
        let mut books = ctx.data_unchecked::<Storage>().lock().await;
        let entry = books.vacant_entry();
        let id: ID = entry.key().into();
        let book = Book {
            id: id.clone(),
            name,
            author,
        };
        entry.insert(book);
        SimpleBroker::publish(BookChanged {
            mutation_type: MutationType::Created,
            id: id.clone(),
        });
        id
    }

    async fn delete_book(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let mut books = ctx.data_unchecked::<Storage>().lock().await;
        let id = id.parse::<usize>()?;
        if books.contains(id) {
            SimpleBroker::publish(BookChanged {
                mutation_type: MutationType::Deleted,
                id: id.into(),
            });
            books.remove(id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Enum, Eq, PartialEq, Copy, Clone)]
enum MutationType {
    Created,
    Deleted,
}

#[derive(Clone)]
struct BookChanged {
    mutation_type: MutationType,
    id: ID,
}

#[Object]
impl BookChanged {
    async fn mutation_type(&self) -> MutationType {
        self.mutation_type
    }

    async fn id(&self) -> &ID {
        &self.id
    }

    async fn book(&self, ctx: &Context<'_>) -> Result<Option<Book>> {
        let books = ctx.data_unchecked::<Storage>().lock().await;
        let id = self.id.parse::<usize>()?;
        Ok(books.get(id).cloned())
    }
}

#[derive(Default)]
pub struct BookSubscriptions;

#[Subscription]
impl BookSubscriptions {
    async fn interval(&self, #[graphql(default = 1)] n: i32) -> impl Stream<Item = i32> {
        let mut value = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        async_stream::stream! {
            loop {
                interval.tick().await;
                value += n;
                yield value;
            }
        }
    }

    async fn books(&self, mutation_type: Option<MutationType>) -> impl Stream<Item = BookChanged> {
        SimpleBroker::<BookChanged>::subscribe().filter(move |event| {
            let res = if let Some(mutation_type) = mutation_type {
                event.mutation_type == mutation_type
            } else {
                true
            };
            async move { res }
        })
    }
}
