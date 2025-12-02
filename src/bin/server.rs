// Server.rs - Collaborative Robots Central Controller
use assignment2::{ClientMessage, RobotState, ServerMessage, BOUNDARY_HEIGHT, BOUNDARY_WIDTH};
use eframe::egui::{self, Color32, Pos2, Rect, CornerRadius, Stroke, Vec2, StrokeKind};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

const BIND_ADDR: &str = "127.0.0.1:5050";
const SAFE_DISTANCE: f32 = 50.0; // Distance to trigger collision warning

// Internal state for a single connected robot
struct RobotData {
    state: RobotState,
    trail: VecDeque<Pos2>,
    last_seen: std::time::Instant,
    // Channel to send commands TO the specific client's writer thread
    tx_to_client: mpsc::Sender<ServerMessage>,
}

// Shared state accessed by GUI and Networking threads
type SharedRobots = Arc<Mutex<HashMap<String, RobotData>>>;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Cobot Central Server",
        options,
        Box::new(|_cc| Ok(Box::new(ServerApp::new()))),
    )
}

struct ServerApp {
    robots: SharedRobots,
    log: Vec<String>,
    rx_log: mpsc::Receiver<String>,
    global_speed_limit: f32,
}

impl ServerApp {
    fn new() -> Self {
        let (tx_log, rx_log) = mpsc::channel();
        let robots = Arc::new(Mutex::new(HashMap::new()));

        let robots_clone = robots.clone();
        let tx_log_clone = tx_log.clone();

        // Spawn Listener Thread
        thread::spawn(move || {
            let listener = TcpListener::bind(BIND_ADDR).expect("Failed to bind");
            let _ = tx_log_clone.send(format!("Server listening on {}", BIND_ADDR));

            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let robots_ref = robots_clone.clone();
                        let log_ref = tx_log_clone.clone();
                        // Spawn a handler per client
                        thread::spawn(move || handle_client(stream, robots_ref, log_ref));
                    }
                    Err(e) => {
                        let _ = tx_log_clone.send(format!("Connection failed: {}", e));
                    }
                }
            }
        });

        Self {
            robots,
            log: vec![],
            rx_log,
            global_speed_limit: 100.0,
        }
    }

    // Novel Feature: Server-side Global Speed Throttle
    fn broadcast_speed_limit(&self) {
        if let Ok(guard) = self.robots.lock() {
            for robot in guard.values() {
                let _ = robot.tx_to_client.send(ServerMessage::SetSpeedLimit(self.global_speed_limit));
            }
        }
    }
}

impl eframe::App for ServerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Process Logs
        while let Ok(msg) = self.rx_log.try_recv() {
            self.log.push(msg);
            if self.log.len() > 50 { self.log.remove(0); }
        }

        egui::SidePanel::left("controls").show(ctx, |ui| {
            ui.heading("Server Controls");
            ui.separator();
            ui.label(format!("Connected Bots: {}", self.robots.lock().unwrap().len()));
            
            ui.separator();
            ui.label("Global Safety Override:");
            if ui.button("EMERGENCY STOP ALL").clicked() {
                if let Ok(guard) = self.robots.lock() {
                    for robot in guard.values() {
                        let _ = robot.tx_to_client.send(ServerMessage::ForceStop);
                    }
                }
                self.log.push("Sent GLOBAL STOP command".into());
            }

            if ui.button("Resume All").clicked() {
                if let Ok(guard) = self.robots.lock() {
                    for robot in guard.values() {
                        let _ = robot.tx_to_client.send(ServerMessage::Resume);
                    }
                }
                self.log.push("Sent RESUME command".into());
            }

            ui.separator();
            ui.label("Global Speed Limit (Novel Feature):");
            if ui.add(egui::Slider::new(&mut self.global_speed_limit, 0.0..=200.0).text("Max Speed")).changed() {
                self.broadcast_speed_limit();
            }

            ui.separator();
            ui.heading("Log");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in &self.log {
                    ui.monospace(line);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Workspace Visualization");
            
            // Allocate a painting region
            let (response, painter) = ui.allocate_painter(
                Vec2::new(BOUNDARY_WIDTH + 50.0, BOUNDARY_HEIGHT + 50.0), 
                egui::Sense::hover()
            );

            // Draw Boundary
            let to_screen = |pos: Pos2| -> Pos2 {
                response.rect.min + Vec2::new(pos.x, pos.y)
            };

            let boundary_rect = Rect::from_min_size(
                to_screen(Pos2::new(0.0, 0.0)), 
                Vec2::new(BOUNDARY_WIDTH, BOUNDARY_HEIGHT)
            );
            
            painter.rect_stroke(boundary_rect, CornerRadius::ZERO, Stroke::new(2.0, Color32::GRAY), StrokeKind::Middle);

            // Logic & Rendering
            if let Ok(guard) = self.robots.lock() {
                // Safety Checks
                let mut ids_to_stop = Vec::new();
                let keys: Vec<String> = guard.keys().cloned().collect();

                // Check collisions between pairs
                for i in 0..keys.len() {
                    for j in (i + 1)..keys.len() {
                        let r1 = &guard[&keys[i]].state;
                        let r2 = &guard[&keys[j]].state;
                        
                        let dist = ((r1.x - r2.x).powi(2) + (r1.y - r2.y).powi(2)).sqrt();
                        
                        // Heatmap / Proximity Warning
                        if dist < SAFE_DISTANCE * 1.5 {
                            // Draw red connection line
                            painter.line_segment(
                                [to_screen(Pos2::new(r1.x, r1.y)), to_screen(Pos2::new(r2.x, r2.y))],
                                Stroke::new(1.0, Color32::RED.linear_multiply(0.5))
                            );
                        }

                        if dist < SAFE_DISTANCE {
                            ids_to_stop.push(keys[i].clone());
                            ids_to_stop.push(keys[j].clone());
                        }
                    }
                }

                // Check Boundaries
                for (id, robot) in guard.iter() {
                    let x = robot.state.x;
                    let y = robot.state.y;
                    if x < 10.0 || x > BOUNDARY_WIDTH - 10.0 || y < 10.0 || y > BOUNDARY_HEIGHT - 10.0 {
                        ids_to_stop.push(id.clone());
                    }
                }

                // Send Stop Commands
                for id in ids_to_stop {
                    if let Some(robot) = guard.get(&id) {
                         // FIXED: Only send stop if the robot is actually active
                         // This prevents spamming the log if the robot is already stopped
                         if robot.state.active {
                             let _ = robot.tx_to_client.send(ServerMessage::ForceStop);
                             let _ = robot.tx_to_client.send(ServerMessage::Warning("Collision/Boundary Risk!".into()));
                         }
                    }
                }

                // Draw Robots
                for robot in guard.values() {
                    let pos = to_screen(Pos2::new(robot.state.x, robot.state.y));
                    let color = Color32::from_rgb(robot.state.color.0, robot.state.color.1, robot.state.color.2);
                    
                    // Draw Trail
                    let points: Vec<Pos2> = robot.trail.iter().map(|p| to_screen(*p)).collect();
                    painter.add(egui::Shape::line(points, Stroke::new(1.0, color.linear_multiply(0.5))));

                    // Draw Robot Body
                    painter.circle_filled(pos, 10.0, color);
                    painter.text(
                        pos + Vec2::new(0.0, -15.0),
                        egui::Align2::CENTER_BOTTOM,
                        &robot.state.id,
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                    
                    if !robot.state.active {
                        painter.text(pos, egui::Align2::CENTER_CENTER, "STOP", egui::FontId::monospace(10.0), Color32::RED);
                    }
                }
            }
        });

        // Constant refresh for animation
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}

fn handle_client(stream: TcpStream, robots: SharedRobots, tx_log: mpsc::Sender<String>) {
    let peer_addr = stream.peer_addr().unwrap().to_string();
    let _ = tx_log.send(format!("New connection: {}", peer_addr));

    // Split stream for full-duplex
    let stream_read = stream.try_clone().expect("Failed to clone stream");
    let mut stream_write = stream;

    // Channel for Server -> Client messages
    let (tx_cmd, rx_cmd) = mpsc::channel::<ServerMessage>();

    // 1. WRITER THREAD: Sends commands to this client
    let log_clone_write = tx_log.clone();
    let peer_addr_clone = peer_addr.clone();

    thread::spawn(move || {
        loop {
            match rx_cmd.recv() {
                Ok(msg) => {
                    let json = serde_json::to_string(&msg).unwrap();
                    if let Err(_) = stream_write.write_all(format!("{}\n", json).as_bytes()) {
                        break; // Client disconnected
                    }
                    let _ = stream_write.flush();
                }
                Err(_) => break, // Channel closed
            }
        }
        let _ = log_clone_write.send(format!("Writer thread ended for {}", peer_addr_clone));
    });

    // 2. READER THREAD (Current thread): Receives telemetry
    let mut reader = BufReader::new(stream_read);
    let mut line = String::new();
    let mut robot_id: Option<String> = None;

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                match serde_json::from_str::<ClientMessage>(&line) {
                    Ok(ClientMessage::Telemetry(state)) => {
                        let mut guard = robots.lock().unwrap();
                        let id = state.id.clone();
                        robot_id = Some(id.clone());

                        let entry = guard.entry(id.clone()).or_insert_with(|| {
                            let _ = tx_log.send(format!("Registered Robot: {}", id));
                            RobotData {
                                state: state.clone(),
                                trail: VecDeque::new(),
                                last_seen: std::time::Instant::now(),
                                tx_to_client: tx_cmd.clone(),
                            }
                        });

                        // Update State
                        entry.state = state.clone();
                        entry.last_seen = std::time::Instant::now();
                        
                        // Update Trail (Keep last 10)
                        entry.trail.push_back(Pos2::new(state.x, state.y));
                        if entry.trail.len() > 20 {
                            entry.trail.pop_front();
                        }
                    },
                    Ok(ClientMessage::Disconnect(id)) => {
                        let _ = tx_log.send(format!("Robot {} sent disconnect.", id));
                        break;
                    },
                    Err(e) => {
                        let _ = tx_log.send(format!("JSON Error from {}: {}", peer_addr, e));
                    }
                }
            }
            Err(_) => break,
        }
    }

    
    // Cleanup
    if let Some(id) = robot_id {
        let mut guard = robots.lock().unwrap();
        guard.remove(&id);
        let _ = tx_log.send(format!("Robot {} removed from state.", id));
    }
}