// EEN1097 Assignment 2
// If required, you can place shared code here that you wish to use in both client.rs and server.rs 

// EEN1097 Assignment 2 - Shared Types
use serde::{Deserialize, Serialize};

pub const BOUNDARY_WIDTH: f32 = 600.0;
pub const BOUNDARY_HEIGHT: f32 = 400.0;

// The state of a single robot, sent from Client -> Server
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RobotState {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub angle: f32,
    pub active: bool,
    // Visual flair: each robot can have a color
    pub color: (u8, u8, u8), 
}

// Messages sent from Client -> Server
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    // Initial handshake or periodic update
    Telemetry(RobotState),
    Disconnect(String),
}

// Messages sent from Server -> Client
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    // Command to force the robot to stop (e.g. collision imminent)
    ForceStop,
    // Command to resume or allow movement
    Resume,
    // Command to set a max speed limit (Global throttle)
    SetSpeedLimit(f32),
    // Informational warning
    Warning(String),
}