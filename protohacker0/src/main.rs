use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn echo_back(stream: &mut TcpStream, buff: &[u8], n: usize) {
    let mut s: usize = 0;
    while s != n {
        s += stream
            .write(&buff[s..n])
            .expect("Server does not want to receive our message");
    }
}

fn handle_echo(mut stream: TcpStream) {
    println!("Handling connection");
    loop {
        let mut buff: Vec<u8> = vec![0; 128];

        let read = stream.read(&mut buff);

        match read {
            Ok(0) => break, // EOF
            Ok(n) => echo_back(&mut stream, &buff, n),
            _ => break, // Unhandled error
        };
    }
}

fn main() -> std::io::Result<()> {
    println!("Starting listener");
    let listener = TcpListener::bind("0.0.0.0:80")?;

    for stream in listener.incoming().flatten() {
        thread::spawn(move || handle_echo(stream));
    }

    Ok(())
}
