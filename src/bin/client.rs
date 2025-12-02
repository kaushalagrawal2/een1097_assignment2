// Client.rs - Cobot Simulator
use assignment2::{ClientMessage, RobotState, ServerMessage, BOUNDARY_HEIGHT, BOUNDARY_WIDTH};
use eframe::egui::{self, Color32, Pos2, CornerRadius, Stroke, Vec2, StrokeKind};
use rand::Rng;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Cobot Client",
        options,
        Box::new(|_cc| Ok(Box::new(ClientApp::new()))),
    )
}

struct ClientApp {
    // Local Simulation State
    state: RobotState,
    target_speed: f32,
    speed_limit: f32, // Controlled by server
    
    // Novel Feature: Wander Mode
    wander_mode: bool,
    last_wander_change: Instant,

    // Communication
    tx_net: Option<mpsc::Sender<ClientMessage>>, // To Network Thread
    rx_net: mpsc::Receiver<ServerMessage>,       // From Network Thread
    connection_status: String,
    
    last_update: Instant,
    logs: Vec<String>,
}

impl ClientApp {
    fn new() -> Self {
        let (_tx_dummy, rx_net) = mpsc::channel();
        
        // Random start position
        let mut rng = rand::thread_rng();
        
        Self {
            state: RobotState {
                id: format!("Cobot-{}", rng.gen_range(100..999)),
                x: rng.gen_range(50.0..300.0),
                y: rng.gen_range(50.0..300.0),
                speed: 0.0,
                angle: rng.gen_range(0.0..6.28),
                active: true,
                // Fix: 'gen' is a keyword in Rust 2024, so we use r#gen
                color: (rng.r#gen(), rng.r#gen(), rng.r#gen()),
            },
            target_speed: 50.0,
            speed_limit: 200.0,
            wander_mode: false,
            last_wander_change: Instant::now(),
            tx_net: None,
            rx_net: rx_net, // Temporary, overwritten on connect
            connection_status: "Disconnected".to_string(),
            last_update: Instant::now(),
            logs: vec!["Welcome. Set ID and Connect.".into()],
        }
    }

    fn connect(&mut self) {
        let address = "127.0.0.1:5050";
        self.connection_status = format!("Connecting to {}...", address);
        
        // Create channels
        let (tx_to_net, rx_from_gui) = mpsc::channel::<ClientMessage>();
        let (tx_to_gui, rx_from_net) = mpsc::channel::<ServerMessage>();
        
        self.tx_net = Some(tx_to_net);
        self.rx_net = rx_from_net;

        let _log_tx = tx_to_gui.clone(); 
        
        thread::spawn(move || {
            match TcpStream::connect(address) {
                Ok(stream) => {
                    let stream_clone = stream.try_clone().expect("Clone failed");
                    
                    // Reader Thread
                    let tx_cmd = tx_to_gui.clone();
                    thread::spawn(move || {
                        let mut reader = BufReader::new(stream_clone);
                        let mut line = String::new();
                        loop {
                            line.clear();
                            match reader.read_line(&mut line) {
                                Ok(0) => break,
                                Ok(_) => {
                                    if let Ok(msg) = serde_json::from_str::<ServerMessage>(&line) {
                                        let _ = tx_cmd.send(msg);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });

                    // Writer Loop (on this thread)
                    let mut writer = stream;
                    loop {
                        match rx_from_gui.recv() {
                            Ok(msg) => {
                                let json = serde_json::to_string(&msg).unwrap();
                                if let Err(_) = writer.write_all(format!("{}\n", json).as_bytes()) {
                                    break;
                                }
                                let _ = writer.flush();
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(e) => {
                    // In a real app we'd signal error back to GUI
                    eprintln!("Failed to connect: {}", e);
                }
            }
        });
        
        self.connection_status = "Connected".to_string();
        self.logs.push("Network threads started.".into());
    }

    fn update_physics(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        if !self.state.active {
            return;
        }

        // Novel Feature: Wander Logic
        if self.wander_mode && now.duration_since(self.last_wander_change).as_secs_f32() > 2.0 {
            let mut rng = rand::thread_rng();
            self.state.angle += rng.gen_range(-1.0..1.0); // Turn slightly
            self.last_wander_change = now;
        }

        // Apply Speed Limit
        let actual_speed = self.target_speed.min(self.speed_limit);
        self.state.speed = actual_speed;

        // Move
        self.state.x += self.state.speed * self.state.angle.cos() * dt;
        self.state.y += self.state.speed * self.state.angle.sin() * dt;

        // Simple local boundary clamp (client side prediction)
        // FIXED: Clamp strictly to visible area (0.0 to WIDTH).
        // The Server triggers alerts at < 10.0 and > WIDTH - 10.0, so hitting 0.0 or WIDTH
        // will successfully trigger the stop logic without the robot disappearing off-screen.
        self.state.x = self.state.x.clamp(0.0, BOUNDARY_WIDTH);
        self.state.y = self.state.y.clamp(0.0, BOUNDARY_HEIGHT);
    }

    fn send_telemetry(&self) {
        if let Some(tx) = &self.tx_net {
            let _ = tx.send(ClientMessage::Telemetry(self.state.clone()));
        }
    }
}

impl eframe::App for ClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Receive Commands
        while let Ok(msg) = self.rx_net.try_recv() {
            match msg {
                ServerMessage::ForceStop => {
                    // FIXED: Only process stop if we are currently active (prevents logic loops)
                    if self.state.active {
                        self.state.active = false;
                        self.state.speed = 0.0;
                        
                        // BOUNCE LOGIC: 
                        // 1. Turn 180 degrees
                        self.state.angle += std::f32::consts::PI; 
                        
                        // 2. Hop slightly away from the wall immediately
                        // This prevents being "stuck" in the wall when you press Go
                        self.state.x += 15.0 * self.state.angle.cos();
                        self.state.y += 15.0 * self.state.angle.sin();
                        
                        // Ensure the hop doesn't push us out of bounds again
                        self.state.x = self.state.x.clamp(0.0, BOUNDARY_WIDTH);
                        self.state.y = self.state.y.clamp(0.0, BOUNDARY_HEIGHT);

                        self.logs.push("CMD: STOPPED (Turned 180° - Press GO to escape)".into());
                    }
                }
                ServerMessage::Resume => {
                    self.state.active = true;
                    self.logs.push("SERVER CMD: RESUME".into());
                }
                ServerMessage::SetSpeedLimit(limit) => {
                    self.speed_limit = limit;
                    self.logs.push(format!("SERVER CMD: Speed Limit {}", limit));
                }
                ServerMessage::Warning(txt) => {
                    self.logs.push(format!("WARNING: {}", txt));
                }
            }
        }

        // 2. Update Physics
        self.update_physics();

        // 3. Send Telemetry (Throttle to ~10Hz)
        if self.tx_net.is_some() && self.last_update.elapsed().as_millis() < 20 {
             self.send_telemetry();
        }

        // 4. GUI Layout
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Cobot Client Controller");
            
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(if self.tx_net.is_some() { "Online" } else { "Offline" });
                ui.colored_label(if self.state.active { Color32::GREEN } else { Color32::RED }, 
                    if self.state.active { "ACTIVE" } else { "STOPPED" });
            });

            ui.separator();
            ui.text_edit_singleline(&mut self.state.id);
            if self.tx_net.is_none() {
                if ui.button("Connect").clicked() {
                    self.connect();
                }
            } else {
                 if ui.button("Disconnect").clicked() {
                     // In a real app we would drop channels
                     self.logs.push("Disconnecting...".into());
                 }
            }

            ui.separator();
            ui.heading("Controls");
            ui.add(egui::Slider::new(&mut self.target_speed, 0.0..=200.0).text("Target Speed"));
            ui.add(egui::Slider::new(&mut self.state.angle, 0.0..=6.28).text("Angle (Rad)"));
            
            ui.horizontal(|ui| {
                if ui.button("Stop").clicked() { self.state.active = false; }
                if ui.button("Go").clicked() { 
                    self.state.active = true; 
                    // Reset speed if it was zeroed out
                    if self.state.speed < 10.0 { self.target_speed = 50.0; } 
                }
            });

            ui.separator();
            ui.checkbox(&mut self.wander_mode, "Wander Mode (Novel Feature)");

            ui.separator();
            ui.label(format!("Pos: ({:.1}, {:.1})", self.state.x, self.state.y));
            
            // Mini Preview
            let (response, painter) = ui.allocate_painter(Vec2::new(300.0, 200.0), egui::Sense::hover());
            let rect = response.rect;
            // Fix: Updated to CornerRadius and added StrokeKind
            painter.rect_stroke(rect, CornerRadius::default(), Stroke::new(1.0, Color32::GRAY), StrokeKind::Middle);
            
            // Map world to mini-preview
            let to_mini = |x: f32, y: f32| -> Pos2 {
                let mx = rect.min.x + (x / BOUNDARY_WIDTH) * rect.width();
                let my = rect.min.y + (y / BOUNDARY_HEIGHT) * rect.height();
                Pos2::new(mx, my)
            };
            
            painter.circle_filled(
                to_mini(self.state.x, self.state.y), 
                5.0, 
                Color32::from_rgb(self.state.color.0, self.state.color.1, self.state.color.2)
            );

            ui.separator();
            egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                for log in self.logs.iter().rev() {
                    ui.monospace(log);
                }
            });
        });
        
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}