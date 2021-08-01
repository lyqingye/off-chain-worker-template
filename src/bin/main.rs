fn main() {}

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
    fn test_event_subscribe() {
        let rt = Arc::new(TokioRuntime::new().unwrap());
        let (mut monitor, event_receiver, _monitor_cmd) = subscribe::monitor::EventMonitor::new(
            "fxdex".to_string().parse().unwrap(),
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
                               info!("{:?}",event_batch);
                               return;
                            },
                            Err(e) => panic!(e.to_string()),
                        }
                }
            }
        }
    }
}
