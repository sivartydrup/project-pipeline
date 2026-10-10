//! Pi RPC transport primitives. RPC stdout is a byte stream of LF-terminated
//! JSON objects; command responses can arrive among unsolicited events.

use crate::{AdapterError, Result};
use serde_json::Value;
use std::collections::HashMap;

const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub struct PiJsonlDecoder {
    pending: Vec<u8>,
}

impl PiJsonlDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns only complete records. An unfinished final record remains
    /// buffered until the next read, and is an error if stdout then closes.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>> {
        self.pending.extend_from_slice(bytes);
        let mut records = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            if end > MAX_RECORD_BYTES {
                return Err(AdapterError::Protocol(
                    "Pi RPC record exceeds size limit".into(),
                ));
            }
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop(); // LF; an optional CR precedes it.
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let record: Value = serde_json::from_slice(&line)
                .map_err(|error| AdapterError::Protocol(format!("invalid Pi RPC JSON: {error}")))?;
            if !record.is_object() {
                return Err(AdapterError::Protocol(
                    "Pi RPC record is not an object".into(),
                ));
            }
            records.push(record);
        }
        if self.pending.len() > MAX_RECORD_BYTES {
            return Err(AdapterError::Protocol(
                "Pi RPC record exceeds size limit".into(),
            ));
        }
        Ok(records)
    }

    pub fn finish(self) -> Result<()> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(AdapterError::Protocol(
                "Pi RPC stdout ended mid-record".into(),
            ))
        }
    }
}

/// Keeps responses by request ID while the caller continues to process events.
/// A response without an ID is a protocol diagnostic, never a match for a
/// pending command.
#[derive(Default)]
pub struct PiRpcInbox {
    responses: HashMap<String, Value>,
}

impl PiRpcInbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn receive(&mut self, record: Value) -> Result<Option<Value>> {
        if record.get("type").and_then(Value::as_str) != Some("response") {
            return Ok(Some(record));
        }
        let Some(id) = record.get("id").and_then(Value::as_str).map(str::to_owned) else {
            return Ok(Some(record));
        };
        if self.responses.contains_key(&id) {
            return Err(AdapterError::Protocol(format!(
                "duplicate Pi RPC response ID: {id}"
            )));
        }
        self.responses.insert(id, record);
        Ok(None)
    }

    pub fn take(&mut self, id: &str, command: &str) -> Result<Option<Value>> {
        let Some(response) = self.responses.remove(id) else {
            return Ok(None);
        };
        if response.get("command").and_then(Value::as_str) != Some(command) {
            return Err(AdapterError::Protocol(format!(
                "Pi RPC response for {id} has wrong command"
            )));
        }
        if response.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(AdapterError::Protocol(
                response
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Pi RPC command failed")
                    .to_owned(),
            ));
        }
        Ok(Some(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn framing_survives_partial_utf8_and_unicode_separators() {
        let mut decoder = PiJsonlDecoder::new();
        let wire = "{\"type\":\"message_update\",\"delta\":\"λ line end\"}\r\n".as_bytes();
        let split = wire.iter().position(|byte| *byte == 0xce).unwrap() + 1;
        assert!(decoder.push(&wire[..split]).unwrap().is_empty());
        let records = decoder.push(&wire[split..]).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["delta"], "λ line end");
        decoder.finish().unwrap();
    }

    #[test]
    fn framing_rejects_truncated_and_invalid_records() {
        let mut decoder = PiJsonlDecoder::new();
        assert!(decoder.push(b"{\"type\":").unwrap().is_empty());
        assert!(decoder.finish().is_err());
        assert!(PiJsonlDecoder::new().push(b"[]\n").is_err());
        assert!(PiJsonlDecoder::new().push(b"not-json\n").is_err());
    }

    #[test]
    fn responses_are_correlated_without_swallowing_events() {
        let mut inbox = PiRpcInbox::new();
        assert_eq!(
            inbox.receive(json!({"type":"agent_start"})).unwrap(),
            Some(json!({"type":"agent_start"}))
        );
        inbox
            .receive(json!({"id":"two","type":"response","command":"get_state","success":true}))
            .unwrap();
        inbox
            .receive(json!({"id":"one","type":"response","command":"prompt","success":true}))
            .unwrap();
        assert!(inbox.take("missing", "prompt").unwrap().is_none());
        assert!(inbox.take("one", "prompt").unwrap().is_some());
        assert!(inbox.take("two", "prompt").is_err());
        assert!(
            inbox
                .receive(json!({"type":"response","command":"parse","success":false}))
                .unwrap()
                .is_some()
        );
    }
}
