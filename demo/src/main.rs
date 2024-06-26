use zksync_basic_types::{L1BatchNumber, U256};
use zksync_types::block::{ BlockGasCount, L1BatchHeader};
use zksync_types::circuit::CircuitStatistic;

use sqlx::postgres::PgPoolOptions;
use sqlx::Error;

use serde_json::Value;

use std::env;
use std::fs::File;
use std::io::{self, Read};
use hex;

use tokio;





#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Dummy Data Insertion in L1Batch Table");

    let files_path = vec![
        "data/pubdata_input_1.txt", 
        "data/pubdata_input_2.txt",
        "data/pubdata_input_3.txt",
        "data/pubdata_input_4.txt",
        "data/pubdata_input_5.txt",
        ];
    
    for (i, file_path) in files_path.iter().enumerate() {
        let pubdata_input = read_pubdata_from_file(file_path)?;
        let block_number: u32 = i as u32 + 1;
        insert_mock_l1_batch( block_number, pubdata_input).await?;
    }

    Ok(())
}

fn read_pubdata_from_file(file_path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)?;

    // Remove "0x" prefix if it exists
    let hex_string = hex_string.trim();
    let hex_string = if hex_string.starts_with("0x") {
        &hex_string[2..]
    } else {
        hex_string
    };
    
    // Convert hex string to Vec<u8>
    let pubdata = hex::decode(hex_string.trim()).expect("Failed to decode hex string");
    Ok(pubdata)
}

async fn insert_mock_l1_batch(batch_number: u32, pub_data: Vec<u8>) -> Result<(), Error> {
    let l1_batch_number = L1BatchNumber(batch_number);
    let mut header  = L1BatchHeader::new(l1_batch_number, 0, Default::default(), Default::default());
    header.pubdata_input = Some(pub_data);

    insert_l1_batch(
        &header,
        &[],
        Default::default(),
        &[],
        &[],
        Default::default(),
    )
    .await
}


async fn insert_l1_batch(
    header: &L1BatchHeader,
    initial_bootloader_contents: &[(usize, U256)],
    predicted_block_gas: BlockGasCount,
    storage_refunds: &[u32],
    pubdata_costs: &[i32],
    predicted_circuits_by_type: CircuitStatistic, // predicted number of circuits for each circuit type
) -> Result<(), Error> {
    let priority_onchain_data: Vec<Vec<u8>> = header
        .priority_ops_onchain_data
        .iter()
        .map(|data| data.clone().into())
        .collect();
    let l2_to_l1_logs: Vec<_> = header
        .l2_to_l1_logs
        .iter()
        .map(|log| log.0.to_bytes().to_vec())
        .collect();
    let system_logs = header
        .system_logs
        .iter()
        .map(|log| log.0.to_bytes().to_vec())
        .collect::<Vec<Vec<u8>>>();
    let pubdata_input = header.pubdata_input.clone().unwrap();
    let initial_bootloader_contents = serde_json::to_value(initial_bootloader_contents).unwrap();
    let used_contract_hashes = serde_json::to_value(&header.used_contract_hashes).unwrap();
    let storage_refunds: Vec<_> = storage_refunds.iter().copied().map(i64::from).collect();
    let pubdata_costs: Vec<_> = pubdata_costs.iter().copied().map(i64::from).collect();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let result = sqlx::query!(
        r#"
        INSERT INTO
            l1_batches (
                number,
                l1_tx_count,
                l2_tx_count,
                timestamp,
                l2_to_l1_logs,
                l2_to_l1_messages,
                bloom,
                priority_ops_onchain_data,
                predicted_commit_gas_cost,
                predicted_prove_gas_cost,
                predicted_execute_gas_cost,
                initial_bootloader_heap_content,
                used_contract_hashes,
                bootloader_code_hash,
                default_aa_code_hash,
                protocol_version,
                system_logs,
                storage_refunds,
                pubdata_costs,
                pubdata_input,
                predicted_circuits_by_type,
                created_at,
                updated_at
            )
        VALUES
            (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                $14,
                $15,
                $16,
                $17,
                $18,
                $19,
                $20,
                $21,
                NOW(),
                NOW()
            )
        "#,
        i64::from(header.number.0),
        i32::from(header.l1_tx_count),
        i32::from(header.l2_tx_count),
        header.timestamp as i64,
        &l2_to_l1_logs,
        &header.l2_to_l1_messages,
        header.bloom.as_bytes(),
        &priority_onchain_data,
        i64::from(predicted_block_gas.commit),
        i64::from(predicted_block_gas.prove),
        i64::from(predicted_block_gas.execute),
        &initial_bootloader_contents,
        &used_contract_hashes,
        header.base_system_contracts_hashes.bootloader.as_bytes(),
        header.base_system_contracts_hashes.default_aa.as_bytes(),
        header.protocol_version.map(|v| v as i32),
        &system_logs,
        &storage_refunds,
        &pubdata_costs,
        &pubdata_input,
        &serde_json::to_value(predicted_circuits_by_type).unwrap(),
    )
    .execute(&pool)
    .await?;

    println!("Inserted l1_batch with result: {:?}", result);

    Ok(())
}