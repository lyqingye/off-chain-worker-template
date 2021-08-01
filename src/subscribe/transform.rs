use crate::events::{Event, NewBlock};
use tendermint::block::Height;
use tendermint::chain::Id as ChainId;
use tendermint_rpc::event::{Event as RpcEvent, EventData as RpcEventData};
pub fn get_all_events(
    _chain_id: &ChainId,
    result: RpcEvent,
) -> Result<Vec<(Height, Event)>, String> {
    let mut vals: Vec<(Height, Event)> = vec![];
    match &result.data {
        RpcEventData::NewBlock { block, .. } => {
            let height: Height = block.as_ref().ok_or("tx.height")?.header.height;
            vals.push((height, NewBlock::new(height).into()));
            let events = &result.events.ok_or("missing events")?;
            println!("events: {:?}", events);
        }

        RpcEventData::Tx { .. } => {
            let events = &result.events.ok_or("missing events")?;
            let _height: Height = events.get("tx.height").ok_or("tx.height")?[0]
                .parse::<Height>()
                .map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(vals)
}
