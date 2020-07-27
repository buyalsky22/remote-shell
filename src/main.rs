use crate::config::{parse_client_config, parse_server_config, ClientConfig, ServerConfig};
use std::env;
use std::io;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::process::{exit, Command, Stdio, Child};
use std::str;

mod config;

const NULL_RESPONSE: &[u8] = " ".as_bytes();
const AUTHENTICATED: &[u8] = "Authenticated".as_bytes();
const FAILED_AUTHENTICATION: &[u8] = "Failed".as_bytes();

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        7 => server(parse_server_config(args)),
        9 => client(parse_client_config(args)),
        _ => {
            println!("Usage for server: -p port -u username -s password");
            println!("Usage for client: -h host_address -p port -u username -s password");
            exit(1);
        }
    };
}

fn server(config: ServerConfig) {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.port as u16);
    let listener = TcpListener::bind(address).unwrap();
    let mut buffer = [0; 1024];

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        println!("New connection: {}", stream.peer_addr().unwrap());
        let mut login_info = [0; 24];

        stream.read(&mut login_info).unwrap();
        let login_info: Vec<String> = String::from_utf8(login_info.to_vec())
            .unwrap()
            .split(" ")
            .map(|s| s.to_string())
            .collect();
        let username = login_info.get(0).unwrap().trim();

        // remove trailing null characters
        let password = login_info.get(1).unwrap().trim_matches(char::from(0));
        if *username == config.username && *password == config.password {
            println!("Authentication succeed!");
            stream.write_all(AUTHENTICATED).unwrap();

            loop {
                let mut previous_command = None;
                let size = stream.read(&mut buffer).unwrap();
                let commands = String::from_utf8_lossy(&buffer[..size]);
                if commands.len() == 0 {
                    break
                }
                let mut commands= commands.trim().split(" | ").peekable();

                while let Some(command) = commands.next() {
                    let stdin = previous_command.map_or(Stdio::inherit(),
                    |output: Child| Stdio::from(output.stdout.unwrap()));
                    let stdout = Stdio::piped();
                    println!("{}", command);
                    let mut parts = command.trim().split_whitespace();
                    let command = parts.next().unwrap();
                    let args = parts;
                    let output = Command::new(command).args(args).stdin(stdin).stdout(stdout).spawn();
                    match output {
                        Ok(output) => { previous_command = Some(output); },
                        Err(e) => {
                            previous_command = None;
                            eprintln!("{}", e);
                        },
                    };
                }
                    match previous_command.unwrap().wait_with_output() {
                        Ok(output) => {
                            let output = str::from_utf8(&output.stdout).unwrap().trim();
                            println!("{:?}", output);
                            if output.len() == 0 {
                                stream.write(NULL_RESPONSE).unwrap();
                            } else {
                                stream.write(output.as_bytes()).unwrap();
                            }
                            stream.flush().unwrap();
                        }
                        Err(e) => {
                            stream
                                .write(format!("Error while executing command ({})", e).as_bytes())
                                .unwrap();
                        }
                    }}

        } else {
            println!("Authentication failed!");
            stream.write_all(FAILED_AUTHENTICATION).unwrap();
        }
    }
}

fn client(config: ClientConfig) {
    let mut buffer = [0; 1024];
    let mut stream =
        TcpStream::connect(format!("{}:{}", config.host_address, config.port)).unwrap();
    println!("Connection established!");
    stream
        .write(format!("{} {}", config.username, config.password).as_bytes())
        .unwrap();

    stream.read(&mut buffer).unwrap();
    // TODO come up with an elegant way
    if buffer[..AUTHENTICATED.len()].to_vec() != AUTHENTICATED.to_vec() {
        println!("Authentication failed!");
        exit(1);
    }

    loop {
        let mut input = String::with_capacity(1024);
        print!("> ");
        io::stdout().flush().unwrap();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        //remove trailing newline
        let command = input.trim();

        if command.ends_with("quit") {
            stream.shutdown(Shutdown::Both).unwrap();
            exit(0);
        }

        stream.write(command.as_bytes()).unwrap();
        let size = stream.read(&mut buffer).unwrap();
        let response = String::from_utf8_lossy(&buffer[..size]);
        if response.len() > 1 {
            println!("{}", response);
        }
    }
}
