use anyhow::Result;
use std::{
    io::{Read, Write},
    net::TcpStream,
    process::Stdio,
    thread::JoinHandle,
};

use chacha20::ChaCha20;
use x25519_dalek::{EphemeralSecret, PublicKey};

fn main() {
    // want to run:
    // ./dav1d -i ~/Rust/rav1e/data/winter-forest-backup.ivf
    // Where: stderr is user facing logging
    // Where: stdout is hidden steg angle bits for processing
    let mut p = std::process::Command::new("../../C/dav1d/build/tools/dav1d");
    // TODO: DONT hardcode!
    p.arg("-i")
        .arg("-")
        .arg("-o")
        .arg("/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = p.spawn().unwrap();
    let mut raw_bits = child.stdout.take().unwrap();
    let stdin = child.stdin.take().unwrap();

    let client_secret = EphemeralSecret::random();
    let client_public = PublicKey::from(&client_secret);

    let mut socket =
        TcpStream::connect("127.0.0.1:6969").expect("Failed to connect to fake RTSP server");

    println!("Connected to server");
    let thread = proxy_av1_data_task(stdin, socket.try_clone().unwrap());

    send_pubkey(&mut socket, &client_public).unwrap();

    let mut server_pubkey = [0u8; 32];
    raw_bits.read_exact(&mut server_pubkey).unwrap();
    println!("Read server pubkey: {:02X?}", &server_pubkey);

    let server_public = PublicKey::from(server_pubkey);

    let shared_secret = client_secret.diffie_hellman(&server_public);
    eprintln!("Shared secret: {:?}", shared_secret.to_bytes());

    let nonce = [0x24; 12];
    use chacha20::cipher::{KeyIvInit, StreamCipher};

    let mut cipher = ChaCha20::new(shared_secret.as_bytes().into(), &nonce.into());

    loop {
        let mut buf = [0u8; 128];
        let n = raw_bits.read(&mut buf).unwrap();
        if n == 0 {
            println!();
            eprintln!("[encountered EOF]");
            break;
        }
        cipher.apply_keystream(&mut buf[..n]);
        let buf = &buf[..n];
        println!("[\"{}\"]", String::from_utf8_lossy(buf));
        if buf.iter().find(|b| **b == 0).is_some() {
            break;
        }
    }

    let s = child.wait().unwrap();
    if !s.success() {
        panic!("dav1d decoder failed");
    }

    thread.join().unwrap().unwrap()
}

fn proxy_av1_data_task(
    mut stdin: std::process::ChildStdin,
    mut socket: TcpStream,
) -> JoinHandle<Result<()>> {
    std::thread::spawn(move || {
        let mut total = 0;
        loop {
            let mut buf = vec![0u8; 8192];
            let Ok(n) = socket.read(&mut buf) else {
                println!("WARNING AV1 copy task exiting after {total} bytes (input read error)");
                break Ok(());
            };
            if n == 0 {
                println!("WARNING AV1 copy task exiting after {total} bytes (input EOF)");
                break Ok(());
            }

            if stdin.write_all(&buf[..n]).is_err() {
                println!("WARNING AV1 copy task exiting after {total} bytes (output write error)");
                break Ok(());
            }
            println!("Wrote {n} bytes to child");
            total += n;
        }
    })
}

fn send_pubkey(socket: &mut TcpStream, client_public: &PublicKey) -> Result<()> {
    let mut buf = vec![];
    buf.push(0b0);
    let param_length: u16 = client_public.as_bytes().len().try_into()?;
    buf.extend(param_length.to_le_bytes());

    buf.extend(client_public.as_bytes());

    buf.push(1);

    socket.write_all(&buf)?;
    socket.flush()?;

    Ok(())
}
