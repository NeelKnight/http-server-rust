use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::prelude::*;
use std::{
    env,
    fs::File,
    io::{BufReader, Read},
    net::{TcpListener, TcpStream},
    thread,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let directory = if let Some(index) = args.iter().position(|arg| arg == "--directory") {
        args.get(index + 1)
            .expect("Directory argument missing")
            .to_string()
    } else {
        "".to_string()
    };

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    println!("Accepted new connection:");
    println!("Listening on http://127.0.0.1:4221...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dir_ref = directory.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &dir_ref) {
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
struct HttpRequest {
    header: String,
    body: String,
}

enum StatusCode {
    BadRequest,
    Created,
    NotFound,
    Ok,
}

impl StatusCode {
    fn text_value(&self) -> &'static str {
        match self {
            StatusCode::BadRequest => "HTTP/1.1 400 Bad Request",
            StatusCode::Created => "HTTP/1.1 201 Created",
            StatusCode::NotFound => "HTTP/1.1 404 Not Found",
            StatusCode::Ok => "HTTP/1.1 200 OK",
        }
    }
}

enum Route<'a> {
    Index,
    Echo(&'a str),
    UserAgent,
    ReadFile(&'a str),
    PostCreateFile(&'a str),
    NotFound,
}

fn parse_route<'a>(method: &str, path: &'a str) -> Route<'a> {
    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => Route::Index,
        ("GET", "/user-agent") => Route::UserAgent,
        ("GET", path) if path.starts_with("/echo/") => Route::Echo(&path["/echo/".len()..]),
        ("GET", path) if path.starts_with("/files/") => Route::ReadFile(&path["/files/".len()..]),
        ("POST", path) if path.starts_with("/files/") => {
            Route::PostCreateFile(&path["/files/".len()..])
        }
        _ => Route::NotFound,
    }
}

fn read_request(stream: &TcpStream) -> HttpRequest {
    let mut buffer = BufReader::new(stream);
    let mut header = String::new();
    let mut line = String::new();

    // Read lines until end of header
    while buffer.read_line(&mut line).unwrap() > 0 {
        if line == "\r\n" {
            break;
        }
        header.push_str(&line);
        line.clear();
    }

    // Check for Content-Length in header
    let mut content_length: usize = 0;
    for header_lines in header.lines() {
        if let Some(value) = header_lines.to_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
            break;
        }
    }

    // If Content-Length is found, read the said bytes
    let body = if content_length > 0 {
        let mut body = String::with_capacity(content_length);
        buffer
            .take(content_length as u64)
            .read_to_string(&mut body)
            .unwrap();
        body
    } else {
        String::new()
    };

    HttpRequest { header, body }
}

fn sanitise_filename(filename: &str, directory: &str) -> String {
    format!("{directory}{filename}")
}

fn read_file(filename: &str) -> std::io::Result<String> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    Ok(contents)
}

fn write_to_file(filename: &str, content: &str) -> std::io::Result<()> {
    let mut file = File::create(filename)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

fn accepts_encoding(header: &str) -> Option<Vec<String>> {
    if let Some(line) = header
        .split("\r\n")
        .find(|line| line.starts_with("Accept-Encoding: "))
    {
        let encodings_str = &line["Accept-Encoding: ".len()..];
        return Some(
            encodings_str
                .split(',')
                .map(|e| e.trim().to_string())
                .collect(),
        );
    }
    None
}

fn encode(content: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    encoder.write_all(content.as_bytes()).unwrap();
    let mut gzip_encoded = encoder.finish().unwrap();

    gzip_encoded[9] = 0x03;

    gzip_encoded
}

// fn bytes_to_str(byte_data: &[u8]) -> String {
//     general_purpose::STANDARD.encode(byte_data)
// }

fn bytes_to_hexstr(byte_data: &[u8]) -> String {
    byte_data
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn structure_response(
    status: StatusCode,
    content_type: &str,
    content_encoding: &str,
    response: &str,
) -> String {
    if response == "" {
        format!("{}\r\n\r\n", status.text_value())
    } else if content_encoding != "" {
        format!(
            "{}\r\nContent-Type: {content_type}\r\nContent-Encoding: {content_encoding}\r\nContent-Length: {}\r\n\r\n{}",
            status.text_value(),
            response.len(),
            response
        )
    } else {
        format!(
            "{}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{}",
            status.text_value(),
            response.len(),
            response
        )
    }
}

fn process_request(request: &HttpRequest, directory: &str) -> String {
    if request.header.contains("GET") || request.header.contains("POST") {
        let request_line: Vec<&str> = request.header.split_whitespace().collect();

        if request_line.len() >= 3 {
            let method = request_line[0];
            let path = request_line[1];

            match parse_route(method, path) {
                Route::Index => {
                    return structure_response(
                        StatusCode::Ok,
                        "text/plain",
                        "",
                        "Welcome to Neel's HTTP Server Project, built with Rust!",
                    )
                }
                Route::Echo(content) => {
                    match accepts_encoding(&request.header) {
                        Some(encodings) => {
                            for encoding in encodings {
                                if encoding == "gzip" {
                                    let encoded_text = bytes_to_hexstr(&encode(content));
                                    return structure_response(
                                        StatusCode::Ok,
                                        "text/plain",
                                        "gzip",
                                        &encoded_text,
                                    );
                                }
                            }
                            return structure_response(
                                StatusCode::Ok,
                                "text/plain",
                                "",
                                "Invalid Encoding / Encoding Not supported by server!",
                            );
                        }
                        None => (),
                    }
                    return structure_response(StatusCode::Ok, "text/plain", "", content);
                }
                Route::UserAgent => {
                    if let Some(line) = request
                        .header
                        .split("\r\n")
                        .find(|line| line.starts_with("User-Agent: "))
                    {
                        let line = &line["User-Agent: ".len()..];
                        return structure_response(StatusCode::Ok, "text/plain", "", line);
                    } else {
                        return structure_response(
                            StatusCode::BadRequest,
                            "text/plain",
                            "",
                            "User Agent Not Found!",
                        );
                    }
                }
                Route::ReadFile(filepath) => {
                    let filename = sanitise_filename(filepath, directory);
                    let content = read_file(&filename);
                    match content {
                        Ok(content) => {
                            return structure_response(
                                StatusCode::Ok,
                                "application/octet-stream",
                                "",
                                &content,
                            )
                        }
                        Err(error) => {
                            let response = format!("File: {filename} NOT found: {error}!");
                            return structure_response(
                                StatusCode::NotFound,
                                "text/plain",
                                "",
                                &response,
                            );
                        }
                    }
                }
                Route::PostCreateFile(filepath) => {
                    if request.body.is_empty() {
                        return structure_response(
                            StatusCode::BadRequest,
                            "text/plain",
                            "",
                            "CreateFile API failure due to HTTP Body not present:!",
                        );
                    }
                    let filename = sanitise_filename(filepath, directory);
                    match write_to_file(&filename, &request.body) {
                        Ok(_) => return structure_response(StatusCode::Created, "", "", ""),
                        Err(error) => {
                            let response = format!("File: {filename} creation failed: {error}!");
                            return structure_response(
                                StatusCode::NotFound,
                                "text/plain",
                                "",
                                &response,
                            );
                        }
                    }
                }
                Route::NotFound => {
                    return structure_response(
                        StatusCode::NotFound,
                        "text/plain",
                        "",
                        "Page NOT found!",
                    )
                }
            }
        }
    }
    structure_response(
        StatusCode::NotFound,
        "text/plain",
        "",
        "Malformed Request Line in HTTP_Request!",
    )
}

fn write_response(mut stream: TcpStream, buffer: &str) -> std::io::Result<()> {
    stream.write_all(buffer.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn handle_connection(stream: TcpStream, directory: &str) -> std::io::Result<()> {
    let http_request = read_request(&stream);
    println!("Request received:\n{http_request:#?}");

    let to_write = process_request(&http_request, directory);
    println!("Ouput to write {}", to_write);
    write_response(stream, &to_write)?;

    Ok(())
}
