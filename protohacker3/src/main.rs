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
const ERR_ALREADY_CONNECTED: &str = "Someone already connected under your name";

enum Event {
    Joined(String),
    Left(String),
    Sent(String, String)
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
fn send_to_all_but(message : &str, but : &str, clients : &mut HashMap<String, TcpStream>) {
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
                    print!("<--- {message}");
                    (message, username)
                },
                Event::Left(username) => {
                    let message = format!("* {username} has left the room\n");
                    print!("<--- {message}");
                    (message, username)
                },
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
fn handshake(stream: &mut TcpStream) -> Result<String, &'static str> {
    send_to_socket(stream, WELCOME_MESSAGE.as_bytes(), WELCOME_MESSAGE.len());

    let username = readline(stream);

    match username {
        Err(m) => Err(m),
        Ok(uname) => {
            if validate_username(&uname) {
                Ok(uname)
            } else {
                Err(ERR_INVALID_USERNAME)
            }
        }
    }
}

fn handle_stream(
    mut stream: TcpStream,
    clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    tx: Sender<Event>,
) {
    println!("New socket opened");

    let username = match handshake(&mut stream) {
        Ok(username) => {
            println!("Validating {username}");
            let mut clients_map = clients.lock().unwrap();

            if clients_map.contains_key(&username) {
                send_to_socket(
                    &mut stream,
                    ERR_ALREADY_CONNECTED.as_bytes(),
                    ERR_ALREADY_CONNECTED.len(),
                );
                return;
            } else {
                let in_room = clients_map.keys().fold(String::new(), |acc, ele| acc + ", " + ele);
                let in_room_message = format!("*Welcome. Users in room: {in_room}\n");
                println!("<-- {in_room_message}");
                send_to_socket(&mut stream, in_room_message.as_bytes(), in_room_message.len());

                clients_map.insert(
                    username.clone(),
                    stream
                        .try_clone()
                        .expect("Unable to clone a TcpSocket... disconnected?"),
                );

                tx.send(Event::Joined(username.clone())).unwrap();
            }

            username
        }
        Err(_) => {
            return;
        }
    };


    loop {
        let line = readline(&mut stream);
        match line {
            Ok(msg) => {
                tx.send(Event::Sent(username.clone(), msg)).unwrap();
            },
            Err(e) => {
                println!("Received error: {e}");

                {
                    let mut clients_map = clients.lock().unwrap();
                    clients_map.remove_entry(&username).unwrap();
                }

                tx.send(Event::Left(username)).unwrap();
                break
            }
        }
    };

}

fn main() -> std::io::Result<()> {
    println!("Starting listener");
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
