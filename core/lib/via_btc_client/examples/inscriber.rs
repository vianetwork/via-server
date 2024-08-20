use via_btc_client::{
    inscriber::Inscriber,
    inscriber::BitcoinNetwork,
    inscriber::NodeAuth
};
use via_btc_client::types as inscribe_types;

use anyhow::{Result, Context};


#[tokio::main]
async fn main () -> Result<()> {
    println!("Hello, world!");
    let mut inscriber = Inscriber::new(
        "url",
        BitcoinNetwork::Testnet,
        NodeAuth::UserPass("via".to_string(), "via".to_string()),
        "prv",
        None,
    ).await.context("Failed to create Inscriber")?;


    println!("balance: {}", inscriber.get_balance().await.context("Failed to get balance")?);


    let l1_da_batch_ref = inscribe_types::L1BatchDAReferenceInput {
        l1_batch_hash: zksync_basic_types::H256([0; 32]),
        l1_batch_index: zksync_basic_types::L1BatchNumber(0_u32),
        da_identifier: "da_identifier_celestia".to_string(),
        blob_id: "temp_blob_id".to_string(),
    };
    
    inscriber.inscribe(inscribe_types::InscriptionMessage::L1BatchDAReference(l1_da_batch_ref)).await.context("Failed to inscribe L1BatchDAReference")?;
    
    Ok(())
}