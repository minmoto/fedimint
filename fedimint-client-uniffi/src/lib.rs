use std::sync::Arc;

use fedimint_client_rpc::{RpcGlobalState, RpcRequest, RpcResponse, RpcResponseHandler};
use fedimint_core::db::Database;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FedimintError {
    #[error("Database error: {msg}")]
    DatabaseError { msg: String },
    
    #[error("Runtime error: {msg}")]
    RuntimeError { msg: String },
    
    #[error("{msg}")]
    Other { msg: String },
}

#[uniffi::export(callback_interface)]
pub trait RpcCallback: Send + Sync {
    /// Called when an RPC response is ready
    fn on_response(&self, response: String);
}

#[derive(uniffi::Object)]
pub struct RpcHandler {
    state: Arc<RpcGlobalState>,
    runtime: tokio::runtime::Runtime,
}

#[uniffi::export]
impl RpcHandler {
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Arc<Self>, FedimintError> {
        let db = create_database(&db_path)
            .map_err(|e| FedimintError::DatabaseError { msg: e.to_string() })?;
        
        // Create the RPC global state (this is where all the fedimint client logic lives)
        let state = Arc::new(RpcGlobalState::new(db));
        
        // Create tokio runtime for async operations
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| FedimintError::RuntimeError { msg: e.to_string() })?;
        
        Ok(Arc::new(Self { state, runtime }))
    }
    
    /// Make an RPC call
    /// 
    /// # Arguments
    /// * `request_json` - JSON string containing the RPC request (see RpcRequest type)
    /// * `callback` - Callback that will receive response JSON strings
    /// 
    /// The callback will be called multiple times:
    /// - Once or more with `{ "type": "data", "data": ... }` responses
    /// - Finally with `{ "type": "end" }` or `{ "type": "error", "error": "..." }`
    pub fn rpc(&self, request_json: String, callback: Box<dyn RpcCallback>) {
        // Parse the JSON request
        let request: RpcRequest = serde_json::from_str(&request_json)
            .expect("Invalid request JSON");
        
        // Handle the RPC call through the global state
        let handled = self.state.clone().handle_rpc(
            request,
            CallbackWrapper(callback)
        );
        
        // If there's an async task to run, spawn it on the runtime
        if let Some(task) = handled.task {
            self.runtime.spawn(task);
        }
    }
}

/// Adapter: Converts UniFFI callback to RpcResponseHandler
/// This bridges the UniFFI world with the fedimint-client-rpc world
struct CallbackWrapper(Box<dyn RpcCallback>);

impl RpcResponseHandler for CallbackWrapper {
    fn handle_response(&self, response: RpcResponse) {
        // Serialize the response to JSON and call the UniFFI callback
        let json = serde_json::to_string(&response)
            .expect("Failed to serialize RPC response");
        self.0.on_response(json);
    }
}

/// Create a database for mobile using fedimint-cursed-redb
/// 
/// Uses redb (pure Rust) instead of RocksDB to avoid C++ dependencies
/// This is more suitable for mobile cross-compilation
fn create_database(path: &str) -> anyhow::Result<Database> {
    use fedimint_cursed_redb::MemAndRedb;
    
    std::fs::create_dir_all(path)?;
    
    let db_path = std::path::Path::new(path).join("fedimint.redb");
    
    let locked_db = tokio::runtime::Runtime::new()?
        .block_on(async { MemAndRedb::new(db_path).await })?;
    
    Ok(Database::new(locked_db, Default::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_handler_creation() {
        let temp_dir = std::env::temp_dir().join("fedimint-test-db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        let handler = RpcHandler::new(temp_dir.to_str().unwrap().to_string());
        assert!(handler.is_ok(), "RpcHandler creation should succeed");
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
