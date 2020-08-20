use std::collections::BTreeMap;
use std::io::prelude::*;
use std::net::TcpStream;
use bytes::{Bytes, BytesMut, BufMut};

#[derive(Debug)]
pub struct Headers {
    entries: BTreeMap<String, String>
}

impl Headers {
    pub fn parse(headers: &[httparse::Header]) -> Headers {
        let mut entries = BTreeMap::new();
        for header in headers.iter() {
            let name = header.name;
            let value = String::from_utf8_lossy(header.value).to_string();
            entries.insert(name.to_lowercase(), value);
        }
        Headers {
            entries
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.entries.get(&key.to_lowercase())
    }
}

#[derive(Debug)]
pub struct Request {
    pub method: Option<String>,
    pub path: Option<String>,
    pub version: Option<u8>,
    pub headers: Headers,
}

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(stream: TcpStream) -> Client {
        Client { stream }
    }

    pub fn read_request(&mut self, buf: &mut BytesMut) -> Result<(Request, BytesMut), std::io::Error> {
        loop {
            let mut lowbuf = [0u8; 2048];
            let len = self.stream.read(&mut lowbuf)?;
            buf.put(&lowbuf[..len]);
            let mut headers = [httparse::EMPTY_HEADER; 100];
            let mut req = httparse::Request::new(&mut headers);
            let res = req.parse(buf.as_ref()).unwrap();
            if let httparse::Status::Complete(len) = res {
                let method = req.method.map(str::to_string);
                let path = req.path.map(str::to_string);
                let version = req.version;
                let headers = Headers::parse(req.headers);
                let result = Request {
                    method,
                    path,
                    version,
                    headers,
                };
                return Ok((result, buf.split_off(len)));
            }
        }
    }

    pub fn read(&mut self) -> Result<(Request, Bytes), std::io::Error> {
        let (request, mut buf) = self.read_request(&mut BytesMut::with_capacity(2048))?;
        buf = buf.split();
        if let Some(length) = request.headers.get("content-length").and_then(|cl| {
            cl.as_str().parse::<usize>().ok()
        }) {
            let mut remaining = BytesMut::with_capacity(length - buf.len());
            remaining.resize(length - buf.len(), 0);
            self.stream.read_exact(remaining.as_mut())?;
            buf.unsplit(remaining);
        }
        Ok((request, buf.freeze()))
    }

    pub fn respond_ok(&mut self, body: Bytes) -> Result<(), std::io::Error> {
        write!(self.stream,
            "HTTP/1.1 200 Ok\r\nContent-Length: {}\r\n\r\n", body.len())?;
        self.stream.write_all(body.as_ref())
    }
}
