use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

enum CommandType {
    Query,
    Insert,
    Invalid,
}

impl CommandType {
    fn parse(data: u8) -> CommandType {
        match data {
            b'I' => CommandType::Insert,
            b'Q' => CommandType::Query,
            _ => CommandType::Invalid,
        }
    }
}

struct Command {
    c_type: CommandType,
    first_number: i32,
    second_number: i32,
}

struct Response {
    value: i32,
}

impl Command {
    fn parse(data: &[u8; 9]) -> Command {
        let c_type = CommandType::parse(data[0]);

        let first_number = slice_to_i32_be(&data[1..5]);
        let second_number = slice_to_i32_be(&data[5..9]);

        Command {
            c_type,
            first_number,
            second_number,
        }
    }

    fn generate_response(&self, datastore: &mut HashMap<i32, i32>) -> Option<Response> {
        match self.c_type {
            CommandType::Query => {
                let earliest = self.first_number;
                let latest = self.second_number;

                let v: Vec<i32> = datastore
                    .iter()
                    .filter(|(&k, _)| earliest <= k && k <= latest)
                    .map(|(_, &v)| v)
                    .collect();

                let sz = v.len();

                let avg = if sz == 0 {
                    0
                } else {
                    let sum = v.iter().fold(0_i64, |acc, &element| acc + (element as i64));
                    (sum / (sz as i64)) as i32
                };

                Some(Response { value: avg })
            }
            CommandType::Insert => {
                let timestamp = self.first_number;
                let value = self.second_number;
                datastore.entry(timestamp).or_insert(value);
                None
            }
            _ => None,
        }
    }
}

fn slice_to_i32_be(data: &[u8]) -> i32 {
    let mut buff: [u8; 4] = [0; 4];

    buff[..4].copy_from_slice(data);

    i32::from_be_bytes(buff)
}

fn send_to_server(stream: &mut TcpStream, buff: &[u8], n: usize) {
    let mut s: usize = 0;
    while s != n {
        s += stream
            .write(&buff[s..n])
            .expect("Failed to send back to the server");
    }
}

fn handle_client(stream: &mut TcpStream) {
    let mut datastore: HashMap<i32, i32> = HashMap::new();

    let mut w: usize = 0;
    let mut buffer: [u8; 9] = [0; 9];
    loop {
        w += match stream.read(&mut buffer[w..9]) {
            Ok(0) => break,  // EOF from server
            Err(_) => break, // What the hell happened
            Ok(amount_read) => amount_read,
        };

        // more data required
        if w < 9 {
            continue;
        }

        // Be ready for next command
        w = 0;

        let cmd = Command::parse(&buffer);
        if let Some(response) = cmd.generate_response(&mut datastore) {
            let response_buff = i32::to_be_bytes(response.value);
            send_to_server(stream, &response_buff, 4);
        }
    }
}

fn handle_stream(mut stream: TcpStream) {
    println!("Handling connection");
    handle_client(&mut stream);
}

fn main() -> std::io::Result<()> {
    println!("Starting listener");
    let listener = TcpListener::bind("0.0.0.0:80")?;

    for stream in listener.incoming().flatten() {
        thread::spawn(move || handle_stream(stream));
    }

    Ok(())
}
