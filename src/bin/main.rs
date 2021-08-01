use off_chain_workers_template::{subscribe, keyring};
use std::sync::Arc;
use tokio::runtime::Runtime as TokioRuntime;
use std::thread;
use crossbeam_channel::select;
use off_chain_workers_template::error::Kind;
use log::info;
use test_env_log::test;

fn main() {
}

#[test]
fn test_key_store() {
    let mut key_store = keyring::KeyRing::new(keyring::Store::Memory,"band","bandchain").unwrap();
    let hdpath = "m/44'/494'/0'/0/0".parse::<keyring::HDPath>().unwrap();
    let key_entry = key_store.key_from_mnemonic("lock nasty suffer dirt dream fine fall deal curtain plate husband sound tower mom crew crawl guard rack snake before fragile course bacon range",&hdpath).unwrap();

    key_store.add_key("example",key_entry.clone()).unwrap();
    let address = hex::encode(key_entry.clone().address);
    println!("{:?}", address);
    println!("{:?}", key_entry.clone());
}

#[test]
fn test_event_subscribe() {
    let rt = Arc::new(TokioRuntime::new().unwrap());
    let (mut monitor,event_receiver,_monitor_cmd) = subscribe::monitor::EventMonitor::new("fxdex".to_string().parse().unwrap(),
                                                                                         "ws://localhost:21657/websocket".parse().unwrap(), rt).unwrap();

    monitor.subscribe().map_err(Kind::EventMonitor).unwrap();
    thread::spawn(move || monitor.run());

    loop{
        select! {
            recv(event_receiver) -> ev_batch => {
                 match ev_batch {
                        Ok(event_batch) => {
                           info!("{:?}",event_batch);
                        },
                        Err(e) => {
                            info!("received error via event bus: {}", e);
                        },
                    }
            }
        }
    }
}
