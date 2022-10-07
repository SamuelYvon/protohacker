use std::cmp::min;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::ops::Deref;

const BUFF_SIZE: usize = 1000;
const NO_VAL_KEY: &str = "";
const VERSION_KEY: &str = "version";

fn index_of_equal(buff: &[u8], sz: usize) -> Option<usize> {
    let mut i = 0;

    while i < sz {
        if buff[i] == b'=' {
            break;
        }
        i += 1;
    }

    if i == sz {
        None
    } else {
        Some(i)
    }
}

fn query(key: String, store: &HashMap<String, String>, buff: &mut [u8]) -> usize {
    println!("Querying for key {key}");
    let dflt = String::from(NO_VAL_KEY);
    let value = store.get(key.deref()).unwrap_or(&dflt);
    println!("Got value {value}");

    let result = format!("{key}={value}");
    let bts = result.as_bytes();

    let total = min(BUFF_SIZE, bts.len());
    buff[..total].copy_from_slice(&bts[..total]);

    total
}

fn insert(buff: &[u8], eq_pos: usize, store: &mut HashMap<String, String>) {
    let key = String::from_utf8(buff[..eq_pos].to_vec()).expect("Should really not fail");
    let value = String::from_utf8(buff[eq_pos + 1..].to_vec()).expect("Should really not fail");

    if key == VERSION_KEY {
        return;
    };

    store.insert(key, value);
}

fn main() -> std::io::Result<()> {
    let mut store: HashMap<String, String> = HashMap::new();
    store.insert(VERSION_KEY.to_string(), "1.0".to_string());

    let mut recv_buff: [u8; BUFF_SIZE] = [0; BUFF_SIZE];
    let mut send_buff: [u8; BUFF_SIZE] = [0; BUFF_SIZE];

    let sock = UdpSocket::bind("0.0.0.0:80")?;

    loop {
        let (sz, addr) = sock.recv_from(&mut recv_buff)?;

        let eq_pos = index_of_equal(&recv_buff, sz);

        if let Some(eq_idx) = eq_pos {
            insert(&recv_buff[..sz], eq_idx, &mut store);
        } else {
            let key = String::from_utf8(recv_buff[..sz].to_vec())
                .expect("Failed to parse string; bad request");
            let sz = query(key, &store, &mut send_buff);
            let reply = &send_buff[..sz];
            sock.send_to(reply, addr).expect("Error while sending, should have worked");
        }

        // reset the buffer
        for val in recv_buff.iter_mut().take(sz) {
            *val = 0;
        }
    };
}
