use std::sync::Arc;
use std::thread;

use crossbeam_channel::select;
use crossbeam_channel::Receiver;
use off_chain_workers_template::subscribe::monitor::Error;
use tokio::runtime::Runtime as TokioRuntime;
use off_chain_workers_template::error::Kind;
use off_chain_workers_template::subscribe;
use off_chain_workers_template::subscribe::monitor::EventBatch;

fn main() {
    env_logger::init();
    let rt = Arc::new(TokioRuntime::new().unwrap());

    let fxdex = create_subscribe(rt.clone(),"ws://localhost:26657/websocket".to_string(),"fxdex".to_string());
    //let band = create_subscribe(rt.clone(),"ws://rpc-laozi-testnet2.bandchain.org:26657/websocket".to_string(),"bandchain".to_string());

    loop {
        select! {
            recv(fxdex) -> ev_batch => {
                 match ev_batch {
                        Ok(_) => {
                           // info!("{:?}",event_batch);
                        },
                        Err(e) => panic!("select event fail, cause by: {}",e.to_string()),
                    }
            },

            // recv(band) -> ev_batch => {
            //      match ev_batch {
            //             Ok(_) => {
            //                // info!("{:?}",event_batch);
            //             },
            //             Err(e) => panic!("select event fail, cause by: {}",e.to_string()),
            //         }
            // },

        }
    }
}

fn create_subscribe(rt: Arc<TokioRuntime>, host: String, chain_name: String) -> Receiver<Result<EventBatch, Error>> {
    let (mut monitor, event_receiver, _monitor_cmd) = crate::subscribe::monitor::EventMonitor::new(
        chain_name.parse().unwrap(),
        host.parse().unwrap(),
        rt,
    )
    .unwrap();

    monitor.subscribe().map_err(Kind::EventMonitor).unwrap();
    thread::spawn(move || monitor.run());
    event_receiver
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use crossbeam_channel::select;
    use tokio::runtime::Runtime as TokioRuntime;
    use tracing::info;

    use off_chain_workers_template::error::Kind;
    use off_chain_workers_template::{keyring, subscribe};

    #[test_env_log::test]
    fn it_works() {
        info!("This record will be captured by `cargo test`");
        assert_eq!(2, 1 + 1);
    }

    #[test_env_log::test]
    fn test_key_store() {
        let mut key_store =
            keyring::KeyRing::new(keyring::Store::Memory, "band", "bandchain").unwrap();
        let hdpath = "m/44'/494'/0'/0/0".parse::<keyring::HDPath>().unwrap();
        let key_entry = key_store.key_from_mnemonic("lock nasty suffer dirt dream fine fall deal curtain plate husband sound tower mom crew crawl guard rack snake before fragile course bacon range", &hdpath).unwrap();

        key_store.add_key("example", key_entry.clone()).unwrap();

        let address = hex::encode(key_entry.clone().address);

        info!("{:?}", address);
        info!("{:?}", key_entry.clone());
    }

    #[test_env_log::test]
    fn test_event_subscribe_fxdex() {
        let rt = Arc::new(TokioRuntime::new().unwrap());
        let (mut monitor, event_receiver, _monitor_cmd) = subscribe::monitor::EventMonitor::new(
            "fxdex".to_string().parse().unwrap(),
            "ws://localhost:26657/websocket".parse().unwrap(),
            rt,
        )
        .unwrap();

        monitor.subscribe().map_err(Kind::EventMonitor).unwrap();
        thread::spawn(move || monitor.run());

        loop {
            select! {
                recv(event_receiver) -> ev_batch => {
                     match ev_batch {
                            Ok(event_batch) => {
                               // info!("{:?}",event_batch);
                            },
                            Err(e) => panic!("select event fail, cause by: {}",e.to_string()),
                        }
                }
            }
        }
    }

    #[test_env_log::test]
    fn test_event_subscribe_band() {
        let rt = Arc::new(TokioRuntime::new().unwrap());
        let (mut monitor, event_receiver, _monitor_cmd) = subscribe::monitor::EventMonitor::new(
            "bandchain".to_string().parse().unwrap(),
            "ws://localhost:21657/websocket".parse().unwrap(),
            rt,
        )
        .unwrap();

        monitor.subscribe().map_err(Kind::EventMonitor).unwrap();
        thread::spawn(move || monitor.run());

        loop {
            select! {
                recv(event_receiver) -> ev_batch => {
                     match ev_batch {
                            Ok(event_batch) => {
                               // info!("{:?}",event_batch);
                            },
                            Err(e) => panic!("select event fail, cause by: {}",e.to_string()),
                        }
                }
            }
        }
    }
}
