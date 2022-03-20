use super::middle::request_middle;
use std::{net::SocketAddr, thread};

use crate::ledger::State;

use super::{eth::EthApiImpl, net::NetApiImpl, web3::Web3ApiImpl};
use web3_rpc_core::{EthApi, NetApi, Web3Api};

pub struct Web3ServerBuilder {
    pub upstream: Vec<SocketAddr>,
    pub http: Vec<SocketAddr>,
    pub ws: Vec<SocketAddr>,
    pub state: State,
}

impl Web3ServerBuilder {
    fn build_http(&self) -> Vec<jsonrpc_http_server::Server> {
        let mut v = vec![];

        for i in 0..self.http.len() {
            let http = self.http.get(i).unwrap();
            let upstream = self.upstream.get(i).unwrap();

            let mut io = jsonrpc_core::IoHandler::new();

            let eth = EthApiImpl {
                upstream: format!("http://{}", upstream),
                state: self.state.clone(),
            };

            let net = NetApiImpl {};

            let web3 = Web3ApiImpl {};

            io.extend_with(eth.to_delegate());
            io.extend_with(net.to_delegate());
            io.extend_with(web3.to_delegate());

            let s = jsonrpc_http_server::ServerBuilder::new(io.clone())
                .request_middleware(request_middle)
                .start_http(http)
                .expect("failed to create http server");
            v.push(s);
        }

        v
    }

    fn build_ws(&self) -> Vec<jsonrpc_ws_server::Server> {
        let mut v = vec![];

        for i in 0..self.ws.len() {
            let ws = self.ws.get(i).unwrap();
            let upstream = self.upstream.get(i).unwrap();

            let mut io = jsonrpc_core::IoHandler::new();

            let eth = EthApiImpl {
                upstream: format!("http://{}", upstream),
                state: self.state.clone(),
            };

            let net = NetApiImpl {};

            let web3 = Web3ApiImpl {};

            io.extend_with(eth.to_delegate());
            io.extend_with(net.to_delegate());
            io.extend_with(web3.to_delegate());

            let s = jsonrpc_ws_server::ServerBuilder::new(io.clone())
                .start(ws)
                .expect("failed to create http server");
            v.push(s);
        }

        v
    }
    pub fn build(self) -> Web3Server {
        let http = self.build_http();

        let ws = self.build_ws();

        Web3Server { http, ws }
    }
}

pub struct Web3Server {
    http: Vec<jsonrpc_http_server::Server>,
    ws: Vec<jsonrpc_ws_server::Server>,
}

impl Web3Server {
    pub fn start(mut self) {
        for _ in 0..self.http.len() {
            let http = self.http.pop().unwrap();
            let ws = self.ws.pop().unwrap();
            let _ = thread::spawn(move || {
                println!("*** Web3-http serve at {} ***", http.address());
                http.wait();
            });

            let _ = thread::spawn(move || {
                println!("*** Web3-websocket serve at {} ***", ws.addr());
                ws.wait().expect("ws start error");
            });
        }
    }
}
