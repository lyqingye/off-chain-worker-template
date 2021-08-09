use crate::{events::{Event, NewBlock}};
use tendermint::block::Height;
use tendermint::chain::Id as ChainId;
use tendermint_rpc::event::{Event as RpcEvent, EventData as RpcEventData};
use std::collections::HashMap;
pub fn extract_rpc_event(
    chain_id: &ChainId,
    result: RpcEvent,
) -> Result<Vec<(Height, Event)>, String> {
    let mut vals: Vec<(Height, Event)> = vec![];
    match &result.data {
        RpcEventData::NewBlock { block, .. } => {
            let height: Height = block.as_ref().ok_or("tx.height")?.header.height;
            vals.push((height, NewBlock::new(height).into()));
            let events = &result.events.ok_or("missing events")?;
            println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
            println!("NewBlock events: {:?}", events);
            println!("-------------------------------------------------------------");
        }

        RpcEventData::Tx{ ..  } => {
            let events = &result.events.ok_or("missing events")?;
            let _height: Height = events.get("tx.height").ok_or("tx.height")?[0]
                .parse::<Height>()
                .map_err(|e| e.to_string())?;


            let actions_and_indices = extract_helper(events)?;
            for action in actions_and_indices {
                match action.0.as_str() {
                    "OracleRequest" => {
                        let seq = events.get("send_packet.packet_sequence").unwrap().get(action.1 as usize).unwrap();
                        let data_str = events.get("send_packet.packet_data").unwrap().get(action.1 as usize).unwrap();
                        let data_json: serde_json::Value = serde_json::from_str(data_str).unwrap();
                        println!("[Dex] => OracleRequest seq: [{}] reqId: [{}] [{}/{}]", 
                            seq, 
                            data_json["client_id"].to_string().replace("\"", "").parse::<u32>().unwrap(),
                            data_json["min_count"].to_string().replace("\"", "").parse::<u32>().unwrap(),
                            data_json["ask_count"].to_string().replace("\"","").parse::<u32>().unwrap());
                    },

                    "report" => {
                        let report_validator = events.get("report.validator").unwrap().get(action.1 as usize).unwrap();
                        let report_id = events.get("report.id").unwrap().get(action.1 as usize).unwrap().to_string().replace("\"","").parse::<u32>().unwrap();

                        println!("[Reporter] => reportId: [{}] validator: [{}]",report_id, report_validator);
                    },

                    "recv_packet" => {
                        let tx_hash = events.get("tx.hash").unwrap().first().unwrap();
                        let src_channel = events.get("recv_packet.packet_src_channel").unwrap()[action.1 as usize].replace("\"","");
                        let src_port = events.get("recv_packet.packet_src_port").unwrap()[action.1 as usize].replace("\"","");
                        let seq = events.get("recv_packet.packet_sequence").unwrap().get(action.1 as usize).unwrap();
                        let pack_data_str = events.get("recv_packet.packet_data").unwrap().get(action.1 as usize).unwrap();
                        let data_json: serde_json::Value = serde_json::from_str(pack_data_str).unwrap();
                        match chain_id.as_str() {
                            "bandchain" => {
                                println!("[Band] <= RecvPacket from [{}:{}] seq: [{}] tx_hash: {}", src_channel,src_port,seq, tx_hash);
                                if let Some(r) = events.get("request.id") {
                                    let req_id = r[action.1 as usize].replace("\"","");
                                    let client_id = events.get("request.client_id").unwrap()[action.1 as usize].replace("\"","");
                                    println!("[Band] ask external ReqId: [{}] ClientId: [{}]",req_id,client_id);
                                }else {
                                    // TODO search cause
                                    println!("[Band] prepare request fail!")
                                }
                            },

                            "fxdex" => {
                                println!("[Dex] <= RecvPacket from [{}:{}] seq: [{}] tx_hash: {}", src_channel,src_port,seq, tx_hash);
                                if let Some(r) = events.get("fx.oracle.MsgOracleRequest.status") {
                                    let status = r[action.1 as usize].replace("\"","");
                                    let client_id = data_json["client_id"].to_string().replace("\"","");
                                    if status == "SUCCESS" {
                                        println!("[Dex] reqId: [{}] solve success! update price: {:?}",client_id, events.get("market_price_updated.market_price").unwrap());
                                    }else {
                                        println!("[Dex] request solve fail!");
                                    }
                                }else {
                                    println!("[Dex] process recv_packet fail")
                                }
                            },
                            _ => {},
                        };
                    },

                    _ => {
                        // println!("++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++");
                        // println!("Tx events: {:?}", events);
                        // println!("-------------------------------------------------------------");
                    }
                }
            }
        }
        _ => {}
    }

    Ok(vals)
}



fn extract_helper(events: &HashMap<String, Vec<String>>) -> Result<Vec<(String, u32)>, String> {
    let actions = events.get("message.action").ok_or("Incorrect Event Type")?;

    let mut val_indices = HashMap::new();
    let mut result = Vec::with_capacity(actions.len());

    for action in actions {
        let idx = val_indices.entry(action.clone()).or_insert_with(|| 0);
        result.push((action.clone(), *idx));

        *val_indices.get_mut(action.as_str()).unwrap() += 1;
    }

    Ok(result)
}

