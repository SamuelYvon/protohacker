use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const VALID_METHOD: &str = "isPrime";
const INVALID_METHOD: &str = "invalid";
const ERR_INVALID_PAYLOAD : & str = "Failed to parse the json payload";
const ERR_INVALID_METHOD : &str = "Error, invalid request method";

#[derive(Deserialize)]
struct ServerRequest {
    method: String,
    number: f64,
}

#[derive(Serialize)]
struct ServerReply {
    method: String,
    prime: bool,
}

fn echo_back(stream: &mut TcpStream, buff: &[u8], n: usize) {
    let mut s: usize = 0;
    while s != n {
        match stream.write(&buff[s..n]) {
            Ok(written_back) => s += written_back,
            Err(_) => panic!("Failed to send back to the server"),
        }
    }
}

fn dumb_is_prime(number: u64) -> bool {
    println!("Checking if prime: {number}");

    if number <= 1 {
        return false;
    } else if number == 2{
        return true;
    }

    let mut divisor = (number as f64).sqrt() as u64;

    while divisor > 1 {
        if number % divisor == 0 {
            return false;
        }
        divisor -= 1;
    }

    true
}

fn check_well_formated_request(request: &ServerRequest) -> std::result::Result<bool, &'static str> { 
    // w/o the static lifetime, the compiler cannot infer the &str does not come from server request
    
    if request.method != VALID_METHOD {
        return Err(ERR_INVALID_METHOD);
    }

    let n = request.number;
    let is_not_float = n.fract() == 0.0 ;

    Ok(is_not_float && dumb_is_prime(n as u64))
}

/// Generate (and send) the response
/// A lot of responsability, but it's just a toy
fn generate_response(stream: &mut TcpStream, request: &[u8]) -> bool {
    let query = String::from_utf8(request.to_vec()).expect("Invalid UTF8 str");
    println!("Received command: |{query}|");

    let request: Result<ServerRequest> = serde_json::from_str(&query);

    let is_prime = match request {
        Ok(req) => check_well_formated_request(&req),
        Err(_) => Err(ERR_INVALID_PAYLOAD)
    };

    println!("Generating response...");

    let mut ok = false;

    let reply: ServerReply = match is_prime {
        Ok(is_prime) => {
            ok = true;
            ServerReply {
                method: VALID_METHOD.to_string(),
                prime: is_prime,
            }
        }
        Err(_) => ServerReply {
            method: INVALID_METHOD.to_string(),
            prime: false,
        },
    };

    let mut reply_buff = serde_json::to_vec(&reply).unwrap();
    reply_buff.push(b'\n'); // responses require newlines

    echo_back(stream, &reply_buff, reply_buff.len());

    ok
}

fn take_requests(stream: &mut TcpStream) {
    let mut w = 0;
    let mut n = 1024;
    let mut buff: Vec<u8> = vec![0; n];

    let mut do_continue = true;

    let mut do_read = true;
    while do_continue {
        println!("Continuing");

        do_continue = loop {
            let read = if do_read { stream.read(&mut buff[w..n]) } else { Ok(0) };

            match (w, read) {
                (0, Ok(0)) => break false, // EOF
                (_, Ok(r)) => {
                    w += r;

                    match buff.iter().position(|&e| e == b'\n') {
                        Some(position_of_newline) => {
                            let reply_ok = generate_response(stream, &buff[0..position_of_newline]);

                            if reply_ok {
                                for (t, i) in (position_of_newline + 1..n).enumerate() {
                                    buff[t] = buff[i];
                                    buff[i] = 0;
                                }

                                w -= position_of_newline + 1;
                            }

                            do_read = w == 0;
                            break reply_ok;
                        }
                        None => {
                            if n > (1 << 15) {
                                panic!("Client is attempting to create a buffer way too big");
                            }

                            n *= 2;
                            buff.resize(n, 0);
                            do_read = true;
                        }
                    }
                }
                (_, Err(_)) => break false,
            }
        }
    }
    println!("Done with thread");
}

fn handle_stream(mut stream: TcpStream) {
    println!("Handling connection");
    take_requests(&mut stream);
}

fn main() -> std::io::Result<()> {
    println!("Starting listener");
    let listener = TcpListener::bind("0.0.0.0:80")?;

    for stream in listener.incoming().flatten() {
        thread::spawn(move || handle_stream(stream));
    }

    Ok(())
}
