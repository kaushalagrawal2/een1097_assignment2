# Assignment 2 Starting Code Template
*EEN1097: Edge Programming with C/C++ and Rust*

## Overview
This project demonstrates a minimal TCP client–server pair with simple graphical user interfaces built using `eframe/egui`.

* The **client** connects to `127.0.0.1:5050`, sends a small JSON message, and displays the server’s reply.
* The **server** listens on `127.0.0.1:5050`, echoes whatever data it receives, and displays the most recent JSON payload in its UI.

The aim is to show how to keep GUI programs responsive while performing blocking TCP I/O by moving networking into a background thread and sending updates back to the GUI using channels.

---

## Repository Structure
```
src/
  main.rs               # Do not use
  lib.rs                # (Optional) shared helpers/types
  bin/
    client.rs           # GUI TCP client (egui)
    server.rs           # GUI TCP echo server (egui)
Cargo.toml
README.md
```

---

## Client (bin/client.rs) — Outline

**Purpose:**
Send a JSON message to the server and display the response using a responsive GUI.

**Key Features:**

* Simple `eframe/egui` UI with one button: **“Connect & Send Message”**.
* There is a radio item that allows you to choose whether the message counts up or down.
* Runs connection, send, and receive logic on a **background thread**.
* The GUI thread receives updates through an **mpsc channel**.
* Adapts the JSON message command in response to the GUI radio box settings.
* Uses a read timeout to avoid blocking indefinitely.
* Clean, minimal structure intended for assignment purposes -- this is not commerical-grade code.

An **mpsc channel** (multi-producer, single-consumer channel) in Rust is a thread-safe communication mechanism that lets multiple threads send messages to one receiving thread without sharing mutable state directly. It provides a `Sender` that can be cloned and used from any thread to push data into the channel, and a `Receiver` that the main thread (often a GUI thread) uses to read those messages in FIFO order. This allows background threads to perform blocking or long-running tasks while safely forwarding results, status updates, or events back to the main thread without risking data races or freezing the application.

---

## Server (bin/server.rs) — Outline

**Purpose:**
Listen for incoming TCP connections, echo all received messages, and display them in an `egui` window.

**Key Features:**

* Binds a `TcpListener` to `127.0.0.1:5050`.
* Serial (single-threaded) connection handling for clarity.
* Displays the latest received message in the GUI, with pretty-printing if it is valid JSON.
* Uses an **mpsc channel** to forward server events to the UI safely by passing commands (e.g., `COUNT_INC`, `COUNT_DEC`)

---

## Building and Running

### 1. Build the project

```bash
cargo build
```

### 2. Run the server (must be first)

```bash
cargo run --bin server
```

### 3. Run the client

```bash
cargo run --bin client
```
---

## Troubleshooting

* **You see a message that cargo is out of date or a feature is missing when running `cargo build`:**
  See the section below on updating `cargo`.

* **The client UI shows no response:**
  Ensure the server is running before starting the client.

* **The server says the port is already in use:**
  Ensure the server has not already been started and if so kill it.

* **Connection refused:**
  Check that the server is running first. 
  Check port `5050`, firewall settings, or antivirus restrictions.

* **Timeouts or no replies:**
  Confirm the server is bound to the correct address and is echoing data correctly.

---
## Updating rustc

Update rustc?

You may need to update your rustc install to buid this project. Take the following steps:

```
PS C:\EEN1097\egui_test> rustc --version
rustc 1.87.0 (17067e9ac 2025-05-09)
PS C:\EEN1097\egui_test> cargo --version
cargo 1.87.0 (99624be96 2025-05-06)
PS C:\EEN1097\egui_test> rustup update stable
…
PS C:\EEN1097\egui_test> rustc --version
rustc 1.91.0 (f8297e351 2025-10-28)
PS C:\EEN1097\egui_test> cargo --version
cargo 1.91.0 (ea2d97820 2025-10-10)
```

---

## Notes for Students

* Remember that GUI code should **never block**.
  Move all blocking operations (networking, file I/O, long computations) into background threads.

* Use channels (`mpsc`) to safely move data between worker threads and the UI.

* Keep your project modular: `lib.rs` is a good place for shared helpers or JSON structs.

* This assignment is a stepping stone toward more advanced patterns: multi-threaded servers, async Rust, and persistent TCP sessions.

---

## Keeping the TCP Connection Alive (High-Level Overview) if necessary

The sample client connects, sends a message, receives a reply, and disconnects.
To maintain a **persistent TCP connection**, the design would need to change slightly:

* Create the `TcpStream` **once** and hold it open for the program’s lifetime.
* Wrap the stream inside `Arc<Mutex<TcpStream>>` to allow safe sharing.
* Use:
  * A **sending channel** (GUI → network thread) for JSON messages.
  * A **receiving channel** (network thread → GUI) for server replies.
* The background thread:
  * Continuously reads from the socket (with timeouts).
  * Writes any JSON messages received from the sending channel.
  * Forwards replies to the GUI using the receiving channel.
* Optionally implement:
  * Application-level keepalives (periodic “ping” JSON).
  * TCP keepalive options (via `socket2` crate).

This approach avoids reconnecting for every message and keeps the GUI smooth.
