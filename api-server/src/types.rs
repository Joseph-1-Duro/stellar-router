use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateRequest {
    /// Target contract address (56-char Stellar contract ID starting with C)
    pub target: String,
    /// Function name to invoke
    pub function: String,
    /// Transaction amount in stroops (used for fee estimation)
    #[serde(default = "default_amount")]
    pub amount: i64,
    /// Fee rate in basis points (default 30 = 0.30%)
    #[serde(default = "default_fee_bps")]
    pub fee_bps: u32,
    /// Network load in basis points for surge pricing (0–10000)
    #[serde(default)]
    pub network_load_bps: u32,
}

fn default_amount() -> i64 { 1_000_000 }
fn default_fee_bps() -> u32 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateResponse {
    pub success: bool,
    pub estimated_fees: FeeEstimate,
    pub simulation: SimulationDetail,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub base_fee: i64,
    pub resource_fee: i64,
    pub total_fee: i64,
    pub surge_multiplier: u32,
    pub high_load: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationDetail {
    pub target: String,
    pub function: String,
    pub would_succeed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
/// Transaction simulation request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateRequest {
    /// Target contract address
    pub target: String,
    /// Function name to invoke
    pub function: String,
    /// Optional route breakdown details
    #[serde(default)]
    pub route_details: Option<RouteDetails>,
}

/// Route breakdown details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDetails {
    /// Route name/identifier
    pub name: String,
    /// Route version
    #[serde(default)]
    pub version: Option<u32>,
    /// Expected output amounts
    #[serde(default)]
    pub expected_outputs: Option<Vec<String>>,
}

/// Transaction simulation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateResponse {
    /// Whether simulation succeeded
    pub success: bool,
    /// Estimated fees in stroops
    pub estimated_fees: FeeEstimate,
    /// Expected output amounts
    pub expected_outputs: Vec<String>,
    /// Route breakdown
    pub route_breakdown: RouteBreakdown,
    /// Human-readable message
    pub message: String,
}

/// Fee estimate details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    /// Base network fee in stroops
    pub base_fee: i64,
    /// Estimated resource fee in stroops
    pub resource_fee: i64,
    /// Total estimated fee in stroops
    pub total_fee: i64,
    /// Surge multiplier (100 = 1x, 200 = 2x)
    pub surge_multiplier: u32,
    /// Whether high-load conditions detected
    pub high_load: bool,
}

/// Route breakdown information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBreakdown {
    /// Route name
    pub route_name: String,
    /// Route version
    pub version: u32,
    /// Target contract address
    pub target_contract: String,
    /// Function being called
    pub function: String,
}

/// Transaction status event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatusEvent {
    /// Transaction ID
    pub tx_id: String,
    /// Current status
    pub status: TransactionStatus,
    /// Timestamp of status update
    pub timestamp: String,
    /// Optional message
    #[serde(default)]
    pub message: Option<String>,
}

/// Transaction status enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionStatus {
    /// Transaction is pending
    Pending,
    /// Transaction submitted to network
    Submitted,
    /// Transaction confirmed on-chain
    Confirmed,
    /// Transaction failed
    Failed,
}

/// WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeMessage {
    /// Action type
    pub action: String,
    /// Transaction ID to subscribe to
    pub tx_id: String,
}

/// WebSocket message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    /// Message type
    pub msg_type: String,
    /// Message data
    pub data: serde_json::Value,
}
