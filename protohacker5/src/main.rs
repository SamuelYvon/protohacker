use regex::Regex;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

const SERVER_EOF: &str = "The client has disconnected";
const SERVER_ERR: &str = "Random error has occured";
const MSG_OUT_OF_RANGE: &str = "The message is too large";
const TONY_BOGUS: &str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";
const TONY_SERVER_URL: &str = "chat.protohackers.com";
const CLIENT_TO_TONY: &str = "[client=>tony]";
const TONY_TO_CLIENT: &str = "[tony=>client]";
const BOGUS_REGEX: &str = r"(7(\w){25,34})";
const TONY_SERVER_PORT: u16 = 16963;

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

fn rewrite_message(regex: &Regex, message: &str) -> String {
    let mut result = message.to_string();
    let matches = regex.find_iter(message);

    let msg_len = message.len();
    let msg_bts = message.as_bytes();

    println!("End = {msg_len}");

    for re_match in matches {
        let s = re_match.start();
        let e = re_match.end();

        // not a match if not at the start and not preceeded by a space
        if 0 < s && msg_bts[s - 1] != b' ' {
            continue;
        }

        // not a match if not at the end and not followed by a space
        if e < msg_len && msg_bts[e] != b' ' {
            continue;
        }

        // a match :)
        let str_re_match = re_match.as_str();
        result = result.replace(str_re_match, TONY_BOGUS);
    }

    result.push('\n');

    result
}

/// Get a connection towards the upstream server
fn tcp_to_tony() -> TcpStream {
    let mut tony_proxy_iter = format!("{TONY_SERVER_URL}:{TONY_SERVER_PORT}")
        .to_socket_addrs()
        .unwrap();
    let sock_addr = tony_proxy_iter.next().unwrap();
    TcpStream::connect(sock_addr).unwrap()
}

fn proxy_and_rewrite(
    name: &str,
    mut source: TcpStream,
    mut target: TcpStream,
    alive: Arc<Mutex<bool>>,
) {
    let re = Regex::new(BOGUS_REGEX).unwrap();
    loop {
        // check if either connection has dropped
        {
            let is_alive = alive.lock().unwrap();
            if !*is_alive {
                println!("{name} IS NOT ALIVE; EXIT");
                break;
            }
        }

        let read_result = readline(&mut source);

        println!("{name} Received message {:?}", read_result);

        if read_result.is_err() {
            println!("{name} Error while reading; closing shop");
            let mut is_alive = alive.lock().unwrap();
            *is_alive = false;

            // this is a bit dirty bc we are closing the write end
            if target.shutdown(std::net::Shutdown::Both).is_err() {
                println!("Failed to shutdown when closing :(");
            };

            break;
        } else if let Ok(message) = read_result {
            // replace with tony's and add the lost newline back
            let new_message = rewrite_message(&re, &message);

            if new_message != message {
                println!(
                    "{name} Received message {:?}, Rewritten as {new_message}",
                    message
                );
            }

            send_to_socket(&mut target, new_message.as_bytes(), new_message.len());
        }
    }
}

fn establish_proxy(client_stream: TcpStream) {
    let alive = Arc::new(Mutex::new(true));

    // > For each client that connects to your proxy server, you'll make a corresponding outward connection to the upstream server
    let tony_stream = tcp_to_tony();

    let tony_read = tony_stream.try_clone().unwrap();
    let client_read = client_stream.try_clone().unwrap();

    {
        let alive = Arc::clone(&alive);
        thread::spawn(move || proxy_and_rewrite(CLIENT_TO_TONY, client_read, tony_stream, alive));
    }

    {
        let alive = Arc::clone(&alive);
        thread::spawn(move || proxy_and_rewrite(TONY_TO_CLIENT, tony_read, client_stream, alive));
    }
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:80")?;

    // Create a client thread for each connection
    for stream in listener.incoming().flatten() {
        establish_proxy(stream);
    }

    Ok(())
}
