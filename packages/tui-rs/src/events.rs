//! R3 EventStream: GET /event (SSE) → typed Events on a channel, reconnecting with backoff.

use std::time::Duration;

use futures::StreamExt;
use reqwest_eventsource::{Event as Sse, EventSource};
use tokio::sync::mpsc;

use crate::api::Api;
use crate::types::Event;

pub enum StreamMsg {
    Event(Event),
    /// (Re)connected: the app should reload state it may have missed.
    Connected,
    Disconnected(String),
}

pub fn subscribe(api: &Api) -> mpsc::UnboundedReceiver<StreamMsg> {
    let (tx, rx) = mpsc::unbounded_channel();
    let url = api.url("/event");
    tokio::spawn(async move {
        let mut backoff = Duration::from_millis(250);
        loop {
            let mut source = EventSource::get(url.clone());
            while let Some(item) = source.next().await {
                match item {
                    Ok(Sse::Open) => {
                        backoff = Duration::from_millis(250);
                        if tx.send(StreamMsg::Connected).is_err() {
                            return;
                        }
                    }
                    Ok(Sse::Message(msg)) => {
                        if let Some(event) = Event::parse(&msg.data) {
                            if tx.send(StreamMsg::Event(event)).is_err() {
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(StreamMsg::Disconnected(err.to_string()));
                        source.close();
                        break;
                    }
                }
            }
            if tx.is_closed() {
                return;
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(5));
        }
    });
    rx
}
