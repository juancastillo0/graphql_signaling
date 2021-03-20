mod models;

use crate::auth::{SessionId, AUTH};
use crate::books::BooksMutations;

use async_graphql::{Context, Error, Object, Result, SimpleObject, Subscription};
use futures::Stream;
use models::{RoomObj, RoomsStorage, Signal, SignalObj};
use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;

static ROOMS: Lazy<Arc<RoomsStorage>> = Lazy::new(Default::default);

fn get_user_id<'a>(ctx: &'a Context<'_>) -> Result<&'a SessionId> {
    ctx.data_opt::<SessionId>()
        .ok_or(Error::new("Unauthorized"))
}

#[derive(SimpleObject)]
struct UserSession {
    user_id: String,
    token: Option<String>,
}

#[derive(Default)]
pub struct SignalQueries;

#[Object]
impl SignalQueries {
    async fn users(&self) -> Vec<String> {
        ROOMS.users.iter().map(|v| v.key().to_string()).collect()
    }

    async fn rooms(&self) -> Vec<String> {
        ROOMS.rooms.iter().map(|v| v.key().to_string()).collect()
    }
}
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn books(&self) -> BooksMutations {
        BooksMutations
    }

    async fn create_session_id(&self, ctx: &Context<'_>) -> Result<UserSession> {
        if let Some(user_id) = ctx.data_opt::<SessionId>() {
            Ok(UserSession {
                user_id: user_id.value().to_string(),
                token: None,
            })
        } else {
            let len = 24;
            let user_id: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(len)
                .map(char::from)
                .collect();
            dbg!(&user_id);

            let (token, cookie) = AUTH.make_cookie(user_id.clone());

            ctx.insert_http_header("Set-Cookie", cookie);

            Ok(UserSession {
                user_id,
                token: Some(token),
            })
        }
    }

    async fn signal(&self, ctx: &Context<'_>, peer_id: String, signal: String) -> Result<bool> {
        let user_id = get_user_id(ctx)?;
        let user = ROOMS.users.get(&peer_id);
        if let Some(obs) = user {
            obs.send(Signal {
                payload: signal,
                peer_id: user_id.to_string(),
            })?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Default)]
pub struct SignalSubscriptions;

#[Subscription]
impl SignalSubscriptions {
    async fn room(
        &self,
        ctx: &Context<'_>,
        room_id: String,
    ) -> Result<impl Stream<Item = RoomObj>> {
        let user_id = get_user_id(ctx)?;

        if !ROOMS.users.contains_key(user_id.as_ref()) {
            return Err(Error::new("You have to listen to signals first"));
        }

        Ok(ROOMS.subscribe_to_room(room_id, user_id.to_string()).await)
    }

    async fn signals(&self, ctx: &Context<'_>) -> Result<impl Stream<Item = SignalObj>> {
        let user_id = get_user_id(ctx)?.to_string();
        Ok(ROOMS.subscribe_to_signals(user_id))
    }
}
