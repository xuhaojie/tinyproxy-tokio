use {log::*, url::Url, anyhow::{*, Result}, dotenv, core::str::Lines, std::net::SocketAddr};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener,TcpStream}, task};

const BUFFER_SIZE: usize = 256;


#[tokio::main]
async fn main() -> Result<()> {
	env_logger::init();
	dotenv::dotenv().ok();
	let server_address = dotenv::var("SERVER_ADDRESS").unwrap_or("0.0.0.0:8088".to_owned());
	
	info!("listening on {}", &server_address);

	let server = TcpListener::bind(server_address).await?;

	while let Result::Ok((client_stream, client_addr)) = server.accept().await {
		debug!("accept client: {}", client_addr);
		task::spawn(async move {
			match process_client(client_stream, client_addr).await { anyhow::Result::Ok(()) => (), Err(e) => error!("error: {}", e), }
		});
	}
	Ok(())
}

fn find_host<'a>(lines: &'a mut Lines) -> Result<&'a str>{
	while let Some(line) = lines.next() {
		let mut fields = line.split(':');
		if let Some(key) = fields.next(){
			if key == "Host" {
				if let Some(value) = fields.next() {
					return Ok(value);
				}
			}
		}
	}
	Err(anyhow!("can't find host"))
}

async fn process_client(mut client_stream: TcpStream, client_addr: SocketAddr) -> Result<()> {

	let mut buf : [u8; BUFFER_SIZE] = unsafe { std::mem::MaybeUninit::uninit().assume_init() }; 

	let count = client_stream.read(&mut buf).await?;
	if count == 0 { return Ok(()); }

	let request = String::from_utf8_lossy(&buf);
	let mut lines = request.lines();
	let line = match lines.next() {Some(l) => l, None => return Err(anyhow!("bad request")) };
	let mut fields = line.split_whitespace();
	let method = match fields.next() {Some(m) => m, None => return Err(anyhow!("can't find request method"))};
	let url_str = match fields.next() {Some(u) =>  u, None => return Err(anyhow!("can't find url"))};

	let (https, address) = match method {
		"CONNECT"  => (true, String::from(url_str)),
		_ => {
			let url =  Url::parse(url_str)?;
			match url.host() {
				Some(addr) => (false, format!("{}:{}", addr.to_string(), url.port().unwrap_or(80))),
				_ => {
					let host = find_host(&mut lines)?;
					error!("get host from head {}", host);
					if host.contains(":") {
						(false, host.to_string())
					} else {
						(false, format!("{}:{}",host,80))
					}
				}
			}
		}
	};

	info!("{} -> {}", client_addr.to_string(), line);

    let mut server_stream = TcpStream::connect(address).await?;


	if https { client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;} 
	else { server_stream.write_all(&buf[..count]).await?; }

    tokio::io::copy_bidirectional(&mut client_stream, &mut server_stream).await?;

	Ok(())
}

