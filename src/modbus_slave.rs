use futures::future;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex,};

use tokio_modbus::prelude::*;
use tokio_modbus::server::{self, Service};

#[derive(PartialEq, Hash, Debug, Clone, Eq, Copy)]
pub enum NodeStat {
    // Init,
    // InitFailed,
    // Sended,
    SendFailed,
    // Recved,
    RecvFailed, // timeout
    // Lost,
    BadData,
    // Idle,
}

pub(crate) struct MbServer {
    rx: Arc<Mutex<Receiver<Option<(usize, Vec<NodeStat>)>>>>,
    id_cnt: usize,
}

trait ModbusTest {
    type OUTPUT;
    fn update(&self, reg: &mut Vec<u16>) -> Self::OUTPUT;
}

impl ModbusTest for MbServer {
    type OUTPUT = Result<Vec<u16>, ()>;
    fn update(&self, reg: &mut Vec<u16>) -> Self::OUTPUT {
        // let mut registers: Vec<u16> = vec![0; self.id_cnt];
        let mut registers = reg.clone();
        match self.rx.lock() {
            Ok(r) => {
                let mut it = r.iter();
                while let Some(Some((id,errors))) = it.next(){
                    registers[id] |= 1 << 0;
                    for x in &errors {
                        match &x {
                            NodeStat::SendFailed => registers[id] &= !(1 << 1),
                            NodeStat::RecvFailed => registers[id] &= !(1 << 2),
                            NodeStat::BadData => registers[id] &= !(1 << 3),
                        };                        
                    }
                    println!("update {:?}{:?}{:?},handle this",id,&errors,registers[id]);
                };
                Ok(registers)
            },
            Err(e) => {
                println!("modbus rx unlock failed: {:?}",e);
                Err(())
            },
        }
    }
}

impl Service for MbServer {
    type Request = Request;
    type Response = Response;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        println!("modbus recv  {:?}",req);
        match req {
            Request::ReadInputRegisters(addr, cnt) => {
                let mut reg: Vec<u16> = vec![15; cnt as usize];
                let a = self.update(&mut reg);
                let data = match a {
                    Ok(registers) => {
                        println!("update ok recv {:?}",reg);
                        println!("update ok recv {:?}",registers);
                        registers[addr as usize..(addr + cnt) as usize].to_vec()
                    },
                    Err(_) => {
                        println!("modbus server update return Err, send deskapp empty vectory");
                        vec![]
                    },
                };
                future::ready(Ok(Response::ReadInputRegisters(data)))
            }
            _ => unimplemented!(),
        }
    }
}

pub(crate) async fn server_context(
    socket_addr: SocketAddr,
    id_cnt: usize,
    rx: Arc<Mutex<Receiver<Option<(usize, Vec<NodeStat>)>>>>,
) {
    println!("Starting up server...");
    let server = server::tcp::Server::new(socket_addr);
    server
        .serve(move || {
            Ok(MbServer {
                rx: rx.clone(),
                id_cnt,
            })
        })
        .await
        .unwrap();
}
