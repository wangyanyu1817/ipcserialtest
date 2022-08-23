extern crate chrono;
extern crate clap;
extern crate serialport;
extern crate tokio_modbus;
#[macro_use]
extern crate lazy_static;
mod modbus_slave;
use std::{
    cmp::Ordering::Equal,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::modbus_slave::server_context;
use chrono::prelude::*;
use clap::{arg, ArgMatches, Command};
use modbus_slave::NodeStat;
use serialport::SerialPort;
use tokio::join;

// static GKEY: [&str] = [
//     "/dev/ttyS0".as_ptr(),
//     "/dev/ttyS1".as_ptr(),
//     "/dev/ttyS2".as_ptr(),
//     "/dev/ttyS3".as_ptr(),
//     "/dev/ttyS4".as_ptr(),
//     "/dev/ttyS5".as_ptr(),
//     "/dev/ttyS6".as_ptr(),
//     "/dev/ttyS7".as_ptr(),
//     "/dev/ttyS8".as_ptr(),
// ];

lazy_static! {
    static ref ARGS: ArgMatches = {
        let ports = serialport::available_ports().expect("No ports found!");
        println!("available ports are ");
        ports.iter().for_each(|p| println!("{}", p.port_name));
        let m = Command::new("serial test app")
            .version("0.3")
            .author("WangYanYu. <yanyu1817@126.com>")
            .about("serial loop to read and write")
            .args(&[
                arg!([seq]...   "A sequence of serial port , i.e. COM1 COM2 COM3")
                    .allow_invalid_utf8(true),
                arg!(-t --timeout [timeout] "Set the length to use as a pos whole num i.e. 20"),
                arg!(-a --address [address] "IP address i.e. 127.0.0.1:5502"),
                arg!(-s --speed [speed] "baud rate, i.e. 115200"),
                arg!(-i --interval [interval] "thread interval ms, i.e. 100"),
                arg!(-r --rollback [rollback] "send back when recv data, if rollback flag is set"),
                arg!(-e --send "send flag"),
                arg!(-p --print "print flag"),
                arg!(-l --len [len] "send buf lengths, i.e. [0x01,0x02]"),
                arg!(-d --data [data] "send data, i.e. 01 02 01"),
            ])
            .get_matches();
        m
    };
}

lazy_static! {
    static ref PORTS: Vec<Arc<Mutex<Box<dyn SerialPort>>>> = {
        let timeout: u64 = ARGS.value_of_t("timeout").unwrap_or(10);
        let speed: u32 = ARGS.value_of_t("speed").unwrap_or(115200);
        println!("Set serial speed {} 💨, timeout {} 💣", speed, timeout);

        let p: Vec<Arc<Mutex<Box<dyn SerialPort>>>> = ARGS
            .values_of_lossy("seq")
            .unwrap()
            .iter()
            .map(|port| {
                let s = serialport::new(port.clone(), speed)
                    .timeout(Duration::from_millis(timeout))
                    .open()
                    .unwrap_or_else(|_| panic!("Failed to open port {}", port));
                Arc::new(Mutex::new(s))
            })
            .collect();
        p
    };
}
// lazy_static! {
//     static ref GKEY1: Vec<&'static str> = {
//         let k = ARGS
//             .values_of_lossy("seq")
//             .unwrap()
//             .iter()
//             .map(|x| {
//                 x
//             }).collect();
//         k.into()
//     };
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let interval: u64 = ARGS.value_of_t("interval").unwrap_or(10);
    let mut handler: Vec<thread::JoinHandle<()>> = Vec::new();
    let (tx, rx) = channel::<Option<(usize, Vec<NodeStat>)>>();
    let ltx = Arc::new(Mutex::new(tx));
    // 串口测试
    if ARGS.is_present("send") {
        println!("send present🚀🚀🚀");
        handler = PORTS
            .iter()
            .enumerate()
            .map(|(id, lp)| {
                println!("Talk on {} 🎤", lp.lock().unwrap().name().unwrap());
                let mut datas: Vec<u8> = vec![];
                if let Ok(s) = ARGS.value_of_t::<String>("data") {
                    datas = s
                        .split(' ')
                        .map(|a| u8::from_str_radix(a, 16).unwrap())
                        .collect();
                }
                let lens: u64 = ARGS.value_of_t("len").unwrap_or(10);
                // println!("datas {:?} len {:?}",&datas, lens);
                one_write_thread(
                    id,
                    lp.clone(),
                    ARGS.is_present("print"),
                    datas,
                    lens,
                    interval,
                    ltx.clone(),
                )
            })
            .collect();
    } else {
        println!("recv present🌏🌏🌏");
        let rollback = ARGS.is_present("rollback");
        handler = PORTS
            .iter()
            .map(|lp| {
                println!("Listen on {} 👂", lp.lock().unwrap().name().unwrap());
                one_read_thread(lp.clone(), interval, ARGS.is_present("print"), rollback)
            })
            .collect();
    };
    if ARGS.is_present("send") && ARGS.is_present("address") {
        let addr = ARGS
            .value_of_t::<String>("address")
            .unwrap()
            .parse()
            .unwrap();
        join!(server_context(
            addr,
            PORTS.iter().count(),
            Arc::new(Mutex::new(rx))
        ));
    };
    handler
        .into_iter()
        .map(|h| h.join().unwrap())
        .for_each(drop);
    Ok(())
}

fn one_read_thread(
    lp: Arc<Mutex<Box<dyn SerialPort>>>,
    interval: u64,
    print_flag: bool,
    roll_back: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        let mut p = lp.lock().unwrap();
        let mut buf: Vec<u8> = vec![0; 50];
        if let Ok(cnt) = p.read(buf.as_mut_slice()) {
            if roll_back {
                p.write_all(&buf[0..cnt]).expect("Write failed");
                if print_flag {
                    println!(
                        "🔄{:?} read and send {:?} {:?}",
                        p.name().unwrap(),
                        &buf[0..cnt],
                        Local.timestamp_millis(Local::now().timestamp_millis())
                    );
                }
            } else {
                println!(
                    "🔄{:?} read  {:?} {:?}",
                    p.name().unwrap(),
                    &buf[0..cnt],
                    Local.timestamp_millis(Local::now().timestamp_millis())
                );
            }
        }
        thread::sleep(Duration::from_millis(interval));
    })
}

fn one_write_thread(
    id: usize,
    lp: Arc<Mutex<Box<dyn SerialPort>>>,
    print_flag: bool,
    datas: Vec<u8>,
    lens: u64,
    interval: u64,
    tx: Arc<Mutex<Sender<Option<(usize, Vec<NodeStat>)>>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        let mut p = lp.lock().unwrap();
        let mut event: Vec<NodeStat> = Vec::new();
        let mut rbuf: Vec<u8> = vec![0; lens.try_into().unwrap()];
        let mut wbuf = (0..lens as u8).into_iter().collect();
        if !datas.is_empty() {
            wbuf = datas.clone();
        }
        if let Err(e) = p.write(&wbuf) {
            if !event.contains(&NodeStat::SendFailed) {
                event.push(NodeStat::SendFailed);
            }
            println!("{:?} {:?}", p.name().unwrap(), e);
        }
        if print_flag {
            println!(
                "➡{:?} send {:?} {:?}",
                p.name().unwrap(),
                &wbuf,
                Local.timestamp_millis(Local::now().timestamp_millis())
            );
        }
        if let Ok(cnt) = p.read(rbuf.as_mut_slice()) {
            if print_flag {
                println!(
                    "⬅{:?} recv {:?} {:?}",
                    p.name().unwrap(),
                    &rbuf[0..cnt],
                    Local.timestamp_millis(Local::now().timestamp_millis())
                );
            }
        } else {
            if !event.contains(&NodeStat::RecvFailed) {
                event.push(NodeStat::RecvFailed);
            }
            println!(
                "⬅{:?} recv timeout {:?}",
                p.name().unwrap(),
                Local.timestamp_millis(Local::now().timestamp_millis())
            );
        }
        if Equal != rbuf.cmp(&wbuf) && !event.contains(&NodeStat::BadData) {
            event.push(NodeStat::BadData);
        };
        // for cnt in 0..rbuf.len() {
        //     if cnt >= wbuf.len() || rbuf[cnt] != wbuf[cnt] {
        //         if !event.contains(&NodeStat::BadData) {
        //             event.push(NodeStat::BadData);
        //         }

        //         break;
        //     }
        // }
        tx.lock().unwrap().send(Some((id, event))).unwrap();
        tx.lock().unwrap().send(None).unwrap();
        thread::sleep(Duration::from_millis(interval));
    })
}
