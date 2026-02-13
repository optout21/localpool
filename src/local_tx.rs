use crate::rpc_command::RpcCommand;

/// A transaction in the local pool
#[derive(Debug, Clone)]
pub struct LocalTx {
    /// Time of submission, unix time (utc)
    // submitted_time: u32,
    /// Target time for broadcasting, absolute time or block height
    /// 0 means no delay.
    /// Semantics is like in lock_time
    target_time: u32,
    /// The serialized transaction
    // tx: String, // TODO Vec<u8>,
    /// The original sumbission RPC
    submitter_rpc: RpcCommand,
}

impl LocalTx {
    pub fn new(_txhex: String, target_time: u32, submitter_rpc: RpcCommand) -> Self {
        Self {
            // submitted_time: 0, // TODO
            target_time,
            // tx: txhex,
            submitter_rpc,
        }
    }

    pub fn target_time(&self) -> u32 {
        self.target_time
    }

    /// Returns a reference to the original RPC command
    pub fn get_command(&self) -> &RpcCommand {
        &self.submitter_rpc
    }
}
