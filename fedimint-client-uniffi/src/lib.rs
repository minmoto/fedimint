use std::sync::Arc;

use fedimint_client_rpc::{RpcGlobalState, RpcRequest, RpcResponse, RpcResponseHandler};
use fedimint_core::db::Database;

uniffi::setup_scaffolding!();

const DB_FILE_NAME: &str = "fedimint.redb";

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FedimintError {
    #[error("Database initialization failed: {msg}")]
    DatabaseError { msg: String },
    
    #[error("Failed to create async runtime: {msg}")]
    RuntimeError { msg: String },
    
    #[error("Invalid request JSON: {msg}")]
    InvalidRequest { msg: String },
    
    #[error("General error: {msg}")]
    General { msg: String },
}

#[uniffi::export(callback_interface)]
pub trait RpcCallback: Send + Sync {
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
        let state = Arc::new(RpcGlobalState::new(db));
        
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| FedimintError::RuntimeError { msg: e.to_string() })?;
        
        Ok(Arc::new(Self { state, runtime }))
    }

    pub fn rpc(&self, request_json: String, callback: Box<dyn RpcCallback>) -> Result<(), FedimintError> {
        let request: RpcRequest = serde_json::from_str(&request_json)
            .map_err(|e| FedimintError::InvalidRequest { msg: e.to_string() })?;
        
        let handled = self.state.clone().handle_rpc(
            request,
            CallbackWrapper(callback)
        );
        
        if let Some(task) = handled.task {
            self.runtime.spawn(task);
        }
        
        Ok(())
    }
}

struct CallbackWrapper(Box<dyn RpcCallback>);

impl RpcResponseHandler for CallbackWrapper {
    fn handle_response(&self, response: RpcResponse) {
        // Serialize the response to JSON and call the UniFFI callback
        let json = serde_json::to_string(&response)
            .expect("Failed to serialize RPC response");
                self.0.on_response(json);
    }
}

/// Creates a redb-based database (pure Rust, no C++ dependencies)
fn create_database(path: &str) -> anyhow::Result<Database> {
    use fedimint_cursed_redb::MemAndRedb;
    
    std::fs::create_dir_all(path)?;
    
    let db_path = std::path::Path::new(path).join(DB_FILE_NAME);
    
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
        match handler {
            Ok(_) => {} // Success
            Err(e) => panic!("RpcHandler creation failed: {}", e),
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
