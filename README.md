##Collaborative Robots (Cobots) Simulation

*EEN1097 Assignment 2: Edge Programming with Rust*

📖 *Overview*

This project implements a robust Client/Server architecture in Rust to simulate a collaborative robot (cobot) workspace.

The Server functions as a central safety controller and visualizer. It renders a real-time 2D view of the workspace, tracks robot movements, and enforces safety protocols (collision avoidance and boundary limits).

The Clients act as independent edge devices. They simulate robot physics locally, stream telemetry to the server, and react to control commands.

The system demonstrates multithreaded networking, shared state management, and immediate mode GUI rendering using egui.

📂 *Repository Structure*

src/
  lib.rs                # Shared Data Protocol (JSON Structs & Enums)
  bin/
    client.rs           # Robot Simulator (GUI + Physics + Networking)
    server.rs           # Central Controller (GUI + Visualization + Safety Logic)
Cargo.toml              # Project Dependencies
README.md               # Documentation


🤖 *Client Application (bin/client.rs)*

Purpose:
Simulates a physical robot moving in a 2D space. It handles local physics calculations and communicates with the server via TCP.

Key Features:

Physics Engine: Calculates position updates based on speed and directional angle ($x += speed * \cos(\theta)$).

Mini-Map: Displays a local preview of the robot's position relative to the workspace.

Smart Safety (Bounce Logic): When the server issues a ForceStop, the client automatically turns 180° and hops away from the boundary to prevent getting stuck.

✨ NOVEL FEATURE: Wander Mode: A toggleable autonomous mode where the robot randomly alters its heading over time, simulating a Roomba-like rover.

🖥️ Server Application (bin/server.rs)

Purpose:
Acts as the central monitoring station. It aggregates telemetry from all clients and visualizes the collective state.

Key Features:

Visualizer: Renders the workspace boundaries, robot positions, and movement trails (last 20 points) using egui::Painter.

Proximity Monitor: Calculates Euclidean distances between all active robots.

Heatmap: Draws dynamic red lines between robots when they approach unsafe distances (< 75px).

Safety Override: Automatically sends ForceStop commands if a collision is imminent (< 50px) or a boundary is breached.

✨ NOVEL FEATURE: Global Fleet Control: Includes a "Global Speed Limit" slider that throttles the maximum speed of all connected clients simultaneously.

⚙️ *Architecture & Design*

Communication Protocol

The system uses a strict JSON contract defined in lib.rs to ensure type safety across the network:

RobotState: Telemetry payload (ID, X, Y, Speed, Angle, Color).

ClientMessage: Upstream messages (e.g., Telemetry, Disconnect).

ServerMessage: Downstream commands (e.g., ForceStop, Resume, SetSpeedLimit).

Concurrency Model

To ensure the GUI remains responsive at 60 FPS, blocking network operations are offloaded:

Server: Spawns a main listener thread. For each new client, it spawns two dedicated threads (Reader/Writer) to handle full-duplex communication.

Client: Runs network I/O on background threads, communicating with the main GUI thread via mpsc channels.

State: Shared state is managed via Arc<Mutex<HashMap<String, RobotData>>>.

🚀 *Building and Running*

Prerequisites

Rust (latest stable)

Cargo

1. Build the Project

cargo build --release


2. Run the Server

The server must be started first to listen for incoming connections.

cargo run --bin server


3. Run Clients (Robots)

Open multiple terminal instances (e.g., 3 separate terminals) and run:

cargo run --bin client


4. Usage

Identity: In each client window, enter a unique ID (e.g., "Bot A", "Bot B") and click Connect.

Drive: Use the sliders to control Speed and Angle.

Wander: Enable "Wander Mode" for autonomous movement.

Safety Test: Drive a robot into a wall or another robot to observe the server's safety override in action.

📦 *Dependencies*

eframe / egui: Immediate mode GUI framework.

serde / serde_json: Serialization for JSON telemetry.

rand: Random number generation for autonomous behavior.