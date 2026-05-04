use crate::{
    error::{Error, Result},
    types::ChatChunk,
};
use eventsource_stream::Eventsource;
use futures::{StreamExt, TryStreamExt};
use serde_json::Value;
use std::sync::Arc;

pub fn parse_sse_stream<T, F>(
    response: reqwest::Response,
    mapper: F,
) -> impl futures::Stream<Item = Result<T>> + Send + 'static
where
    T: Send + 'static,
    F: Fn(String) -> Result<Option<T>> + Send + Sync + 'static,
{
    let mapper = Arc::new(mapper);
    response
        .bytes_stream()
        .eventsource()
        .map_err(|err| Error::Stream(err.to_string()))
        .and_then(move |event| {
            let data = event.data;
            let mapper = Arc::clone(&mapper);
            async move {
                if data.trim() == "[DONE]" {
                    return Ok(None);
                }
                (mapper)(data)
            }
        })
        .filter_map(|item| async move {
            match item {
                Ok(Some(value)) => Some(Ok(value)),
                Ok(None) => None,
                Err(err) => Some(Err(err)),
            }
        })
}

pub fn openai_chunk_mapper(data: String) -> Result<Option<ChatChunk>> {
    let raw: Value = serde_json::from_str(&data)?;
    let choice = raw
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());

    let delta = choice
        .and_then(|value| value.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let finish_reason = choice
        .and_then(|value| value.get("finish_reason"))
        .and_then(Value::as_str)
        .map(super::openai::map_finish_reason);

    Ok(Some(ChatChunk {
        done: finish_reason.is_some(),
        delta,
        finish_reason,
        raw: Some(raw),
    }))
}

pub fn claude_chunk_mapper(data: String) -> Result<Option<ChatChunk>> {
    let raw: Value = serde_json::from_str(&data)?;
    let event_type = raw.get("type").and_then(Value::as_str).unwrap_or_default();

    if event_type == "message_stop" {
        return Ok(Some(ChatChunk {
            done: true,
            delta: String::new(),
            finish_reason: Some(crate::types::FinishReason::Stop),
            raw: Some(raw),
        }));
    }

    let delta = raw
        .get("delta")
        .and_then(|delta| delta.get("text"))
        .and_then(Value::as_str)
        .or_else(|| {
            raw.get("content_block")
                .and_then(|block| block.get("text"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_string();

    if delta.is_empty()
        && event_type != "content_block_delta"
        && event_type != "content_block_start"
    {
        return Ok(None);
    }

    Ok(Some(ChatChunk {
        done: false,
        delta,
        finish_reason: None,
        raw: Some(raw),
    }))
}
