#[allow(unused_imports)]
use std::io::prelude::*;
use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    println!("Accepted new connection:");
    println!("Listening on http://127.0.0.1:4221...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_connection(stream).unwrap();
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }
    }
}

fn read_request(stream: &TcpStream) -> Vec<String> {
    let buffer = BufReader::new(stream);
    let http_request: Vec<_> = buffer
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    http_request
}

fn process_request(http_request: &Vec<String>) -> String {
    let first_line = http_request.get(0).unwrap();
    if first_line.contains("GET") {
        let request_parts: Vec<&str> = first_line.split_whitespace().collect();

        if request_parts.len() >= 3 {
            let request_target = request_parts[1];
            match request_target {
                "/index.html" | "/" => return "HTTP/1.1 200 OK\r\n\r\n".to_string(),
                path if path.starts_with("/echo/") => {
                    let content = path.strip_prefix("/echo/").unwrap_or("");
                    return format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                        content.len(), content
                    );
                }
                "/user-agent" => {
                    match http_request
                        .iter()
                        .find(|line| line.contains("User-Agent: "))
                    {
                        Some(line) => {
                            let line = line.strip_prefix("User-Agent: ").unwrap_or("");
                            return format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{line}", line.len());
                        }
                        None => return format!("HTTP/1.1 404 Not Found\r\n\r\n"),
                    }
                }
                _ => return "HTTP/1.1 404 Not Found\r\n\r\n".to_string(),
            }
        }
    }
    "Malformed Request Line in HTTP_Request!".to_string()
}

fn write_request(mut stream: TcpStream, buffer: &str) -> std::io::Result<()> {
    stream.write_all(buffer.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn handle_connection(stream: TcpStream) -> std::io::Result<()> {
    let http_request = read_request(&stream);
    println!("Request received:\n{http_request:#?}");

    let to_write = process_request(&http_request);
    write_request(stream, &to_write)?;

    Ok(())
}
