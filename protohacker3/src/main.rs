use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

const WELCOME_MESSAGE: &str = "Welcome to budgetchat! What shall I call you?\n";
const SERVER_EOF: &str = "The client has disconnected";
const SERVER_ERR: &str = "Random error has occured";
const MSG_OUT_OF_RANGE: &str = "The message is too large";
const ERR_INVALID_USERNAME: &str = "Invalid username";

enum Event {
    Joined(String),
    Left(String),
    Sent(String, String),
}

fn send_to_socket(stream: &mut TcpStream, buff: &[u8], n: usize) {
    let mut s: usize = 0;
    while s != n {
        s += stream
            .write(&buff[s..n])
            .expect("Failed to send back to the server");
    }
}

// Read a line from a Tcp socket. The maximum line line length is 1024 characters.
// Beyond this length an error will be returned. An error may be returned if the socket
// is closed on the sender's end.
fn readline(stream: &mut TcpStream) -> Result<String, &'static str> {
    let mut buff: [u8; 1024] = [0; 1024];
    let mut n = 0;

    while n < 1024 {
        let old_n = n;
        n += match stream.read(&mut buff[n..]) {
            Ok(0) => return Err(SERVER_EOF),
            Err(_) => return Err(SERVER_ERR),
            Ok(n) => n,
        };

        if let Some(idx) = buff[old_n..n].iter().position(|&b| b == b'\n') {
            let result =
                String::from_utf8(buff[..old_n + idx].to_vec()).expect("No reason this fails");
            return Ok(result);
        }
    }

    Err(MSG_OUT_OF_RANGE)
}

fn validate_username(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    if !s.is_ascii() {
        return false;
    }

    s.chars().all(|x| x.is_alphanumeric())
}

// Send the message to all clients except the one specified in the `but` field.
fn send_to_all_but(message: &str, but: &str, clients: &mut HashMap<String, TcpStream>) {
    for (username, stream) in clients.iter_mut() {
        if username != but {
            send_to_socket(stream, message.as_bytes(), message.len());
        }
    }
}

fn sender_thread(clients: Arc<Mutex<HashMap<String, TcpStream>>>, rx: Receiver<Event>) {
    loop {
        let event = rx.recv().unwrap();

        {
            let mut clients_map = clients.lock().unwrap();
            let (message, username) = match event {
                Event::Joined(username) => {
                    let message = format!("* {username} has joined the room\n");
                    (message, username)
                }
                Event::Left(username) => {
                    let message = format!("* {username} has left the room\n");
                    (message, username)
                }
                Event::Sent(username, message) => {
                    let message = format!("[{username}] {message}\n");
                    (message, username)
                }
            };

            send_to_all_but(&message, &username, &mut clients_map);
        }
    }
}

/// Perform the handshake with the new client: ask for the username and validate it.
fn handshake(stream: &mut TcpStream, clients : &Arc<Mutex<HashMap<String, TcpStream>>>) -> Result<String, &'static str> {
    send_to_socket(stream, WELCOME_MESSAGE.as_bytes(), WELCOME_MESSAGE.len());

    let username = readline(stream);

    match username {
        Err(m) => Err(m),
        Ok(uname) => {
            let clients_map = clients.lock().unwrap();
            if validate_username(&uname) && !clients_map.contains_key(&uname) {
                Ok(uname)
            } else {
                Err(ERR_INVALID_USERNAME)
            }
        }
    }
}

/// Receives messages from a client and distributes them upon reception
fn receive_messages(
    mut stream: TcpStream,
    tx: Sender<Event>,
    clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    username: String,
) {
    loop {
        let line = readline(&mut stream);

        match line {
            Ok(msg) => tx.send(Event::Sent(username.clone(), msg)).unwrap(),
            Err(_) => break,
        }
    }

    let mut clients_map = clients.lock().unwrap();
    clients_map.remove_entry(&username).unwrap();
    tx.send(Event::Left(username)).unwrap();
}

/// Send the list of members in the room to the stream
fn send_room_description(
    stream: &mut TcpStream,
    clients: &mut Arc<Mutex<HashMap<String, TcpStream>>>,
) {
    let clients_map = clients.lock().unwrap();

    let in_room = clients_map
        .keys()
        .fold(String::new(), |acc, ele| acc + ", " + ele);

    let in_room_message = format!("*Welcome. Users in room: {in_room}\n");

    send_to_socket(stream, in_room_message.as_bytes(), in_room_message.len());
}

fn handle_stream(
    mut stream: TcpStream,
    mut clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    tx: Sender<Event>,
) {
    let username = match handshake(&mut stream, &clients) {
        Ok(username) => username,
        Err(_) => return
    };

    send_room_description(&mut stream, &mut clients);

    {
        // insert the client in the map and send the joined event
        let mut clients_map = clients.lock().unwrap();
        clients_map.insert(
            username.to_string(),
            stream
                .try_clone()
                .expect("Unable to clone a TcpSocket... disconnected?"),
        );
        tx.send(Event::Joined(username.to_string())).unwrap();
    }

    receive_messages(stream, tx, clients, username);
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:80")?;

    let (tx, rx): (Sender<Event>, Receiver<Event>) = channel();
    let clients: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    // Create the sender thread
    {
        let clients = Arc::clone(&clients);
        thread::spawn(move || sender_thread(clients, rx));
    }

    // Create a client thread for each connection
    for stream in listener.incoming().flatten() {
        let tx = tx.clone();
        let clients = Arc::clone(&clients);
        thread::spawn(move || handle_stream(stream, clients, tx));
    }

    Ok(())
}
