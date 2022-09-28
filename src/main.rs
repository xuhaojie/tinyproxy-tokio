use {log::*, url::Url, anyhow::{*, Result}, dotenv, std::net::SocketAddr};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener,TcpStream}, task};

const BUFFER_SIZE: usize = 256;


#[tokio::main(flavor = "current_thread")] // best benchmark socre, performance much better then go and cpu useage is lower
//#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
	env_logger::init();
	dotenv::dotenv().ok();
	let server_address = dotenv::var("PROXY_ADDRESS").unwrap_or("0.0.0.0:8088".to_owned());
	info!("tinyproxy listening on {}", &server_address);
	let server = TcpListener::bind(server_address).await?;
	while let Result::Ok((client_stream, client_addr)) = server.accept().await {
		task::spawn(async move {
			match process_client(client_stream, client_addr).await { anyhow::Result::Ok(()) => (), Err(e) => error!("error: {}", e), }
		});
	}
	Ok(())
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
				_ => return Err(anyhow!("can't find host from url")),
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
