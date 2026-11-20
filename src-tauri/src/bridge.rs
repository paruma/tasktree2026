use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::ipc::Channel;
use tokio::sync::oneshot;

/// パイプで許可する操作。コード実行・ファイル操作は受け付けない。
#[derive(Debug, Deserialize, Serialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Operation {
    Get {},
    Add {
        title: String,
        #[serde(default = "default_minutes")]
        minutes: Option<u32>,
        #[serde(default)]
        state: u8,
        #[serde(default)]
        parent_id: Option<String>,
    },
    Update {
        id: String,
        patch: Value,
    },
    Move {
        id: String,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default)]
        before_id: Option<String>,
    },
    Remove {
        id: String,
    },
    Undo {},
    Redo {},
}

fn default_minutes() -> Option<u32> {
    Some(30)
}

#[derive(Clone, Serialize)]
pub struct Envelope {
    id: u64,
    deadline: u64,
    request: Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Reply {
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

/// 保存と画面更新の完了応答を、元のパイプ接続へ返す。
#[derive(Default)]
pub struct Bridge {
    channel: Mutex<Option<Channel<Envelope>>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    sequence: AtomicU64,
}

impl Bridge {
    pub fn connect(&self, channel: Channel<Envelope>) {
        *self.channel.lock().unwrap() = Some(channel);
        for (_, sender) in self.pending.lock().unwrap().drain() {
            let _ = sender.send(Reply::error(
                "画面が再読み込みされました。結果を確認してください",
            ));
        }
    }

    pub fn complete(&self, id: u64, reply: Reply) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .unwrap()
            .remove(&id)
            .ok_or("応答先が見つからないか、有効期限が切れています")?;
        sender
            .send(reply)
            .map_err(|_| "接続が終了しています".into())
    }

    pub async fn dispatch(&self, request: Operation) -> Reply {
        let Some(channel) = self.channel.lock().unwrap().clone() else {
            return Reply::error("画面の準備ができていません");
        };
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let deadline = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
            + 10_000;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, sender);
        let envelope = Envelope {
            id,
            deadline,
            request: serde_json::to_value(request).unwrap(),
        };
        if channel.send(envelope).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Reply::error("画面に操作を送信できませんでした");
        }
        let result = tokio::time::timeout(Duration::from_secs(10), receiver).await;
        self.pending.lock().unwrap().remove(&id);
        match result {
            Ok(Ok(reply)) => reply,
            _ => Reply::error("応答がありません。再実行する前に現在のタスクを確認してください"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_known_operations_and_fields() {
        assert!(serde_json::from_str::<Operation>(r#"{"op":"get"}"#).is_ok());
        assert!(serde_json::from_str::<Operation>(r#"{"op":"eval","code":"alert(1)"}"#).is_err());
        assert!(serde_json::from_str::<Operation>(r#"{"op":"get","code":"x"}"#).is_err());
        assert!(
            serde_json::from_str::<Operation>(r#"{"op":"add","title":"x","minutes":-1}"#).is_err()
        );
        let add: Operation = serde_json::from_str(r#"{"op":"add","title":"x"}"#).unwrap();
        assert_eq!(serde_json::to_value(add).unwrap()["minutes"], 30);
    }
}
