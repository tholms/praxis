pub mod manager;
pub mod executor;
pub mod chain_execution;

pub use manager::SemanticOpsManager;
pub use executor::ResponseTracker;
#[allow(unused_imports)]
pub use executor::{execute_one_shot, execute_agent_mode, select_agent, create_session, close_session};
pub use chain_execution::ChainExecutor;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

//
// Per-node execution lock. Both semantic operations and chain executions
// must acquire this lock before running on a node. This ensures only one
// operation or chain runs on a node at any time.
//

#[derive(Clone)]
pub struct NodeExecLock {
    locks: Arc<std::sync::RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

impl NodeExecLock {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, node_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.write().unwrap();
        locks.entry(node_id.to_string()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }
}
