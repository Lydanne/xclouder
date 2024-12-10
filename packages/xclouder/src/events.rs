use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::Value;
use std::collections::HashMap;

pub type EventCallback = Box<dyn Fn(Value) + Send + Sync>;

pub struct Emitter {
    events: Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    last_emit_args: Arc<Mutex<HashMap<String, Value>>>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HashMap::new())),
            last_emit_args: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn emit(&self, event: &str, args: Value) {
        let mut last_args = self.last_emit_args.lock().await;
        last_args.insert(event.to_string(), args.clone());

        if let Some(callbacks) = self.events.lock().await.get(event) {
            for callback in callbacks {
                callback(args.clone());
            }
        }
    }

    pub async fn on(&self, event: &str, callback: EventCallback) {
        let mut events = self.events.lock().await;
        let callbacks = events.entry(event.to_string()).or_insert_with(Vec::new);
        
        // 如果有上次的事件数据，立即调用
        if let Some(args) = self.last_emit_args.lock().await.get(event) {
            callback(args.clone());
        }
        
        callbacks.push(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_event_emitter() {
        let emitter = Emitter::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        emitter.on("test", Box::new(move |args| {
            let received = received_clone.clone();
            tokio::spawn(async move {
                received.lock().await.push(args);
            });
        })).await;

        let test_data = serde_json::json!({"message": "hello"});
        emitter.emit("test", test_data.clone()).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let received_data = received.lock().await;
        assert_eq!(received_data.len(), 1);
        assert_eq!(received_data[0], test_data);
    }
} 