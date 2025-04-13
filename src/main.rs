#[allow(unused_imports)]
use std::io::prelude::*;
use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    thread,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    println!("Accepted new connection:");
    println!("Listening on http://127.0.0.1:4221...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    if let Err(error) = handle_connection(stream) {
                        eprintln!("Error handling connection: {}", error);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }
    }
}

#[derive(Debug)]
enum StatusCode {
    Ok,
    NotFound,
}

impl StatusCode {
    fn text_value(&self) -> &'static str {
        match self {
            StatusCode::Ok => "HTTP/1.1 200 OK",
            StatusCode::NotFound => "HTTP/1.1 404 Not Found",
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
                "/index.html" | "/" => return structure_response(StatusCode::Ok, ""),
                path if path.starts_with("/echo/") => {
                    let content = path.strip_prefix("/echo/").unwrap_or("");
                    return structure_response(StatusCode::Ok, content);
                }
                "/user-agent" => {
                    if let Some(line) = http_request
                        .iter()
                        .find(|line| line.contains("User-Agent: "))
                    {
                        let line = line.strip_prefix("User-Agent: ").unwrap_or("");
                        return structure_response(StatusCode::Ok, line);
                    }
                }
                _ => return structure_response(StatusCode::NotFound, "Page NOT found!"),
            }
        }
    }
    structure_response(
        StatusCode::NotFound,
        "Malformed Request Line in HTTP_Request!",
    )
}

fn structure_response(status: StatusCode, response: &str) -> String {
    format!(
        "{}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        status.text_value(),
        response.len(),
        response
    )
}

fn write_response(mut stream: TcpStream, buffer: &str) -> std::io::Result<()> {
    stream.write_all(buffer.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn handle_connection(stream: TcpStream) -> std::io::Result<()> {
    let http_request = read_request(&stream);
    println!("Request received:\n{http_request:#?}");

    let to_write = process_request(&http_request);
    println!("Ouput to write {}", to_write);
    write_response(stream, &to_write)?;

    Ok(())
}
