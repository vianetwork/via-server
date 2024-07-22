
use bitcoin::blockdata::opcodes::all::{OP_CHECKSIG, OP_ENDIF, OP_FALSE, OP_IF};
use bitcoin::blockdata::script::Builder;
use bitcoin::blockdata::transaction::{Transaction, TxIn, TxOut};
use bitcoin::consensus::encode::{serialize, deserialize};
use bitcoin::Network;
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::Address;
use bitcoin::sighash::SighashCache;
use bitcoin::PrivateKey;
use bitcoin::taproot::{TapLeafHash, TaprootBuilder};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use std::str::FromStr;


// create a Inscription transaction
// Flow:
//      1. unlock all available UTXOs for the source address
//      2. create inscription output with using Taproot approach (stack data): 
//            - **PUBKEY** 
//            - OP_CHECKSIG 
//            - OP_FALSE OP_IF 
//            - **INSCRIPTION DATA** 
//            - OP_ENDIF
//     3. create a P2WPKH change output to send the remaining funds back to the source address
//     4. create a transaction with the inputs and outputs
//     5. sign the transaction with the private key
//     6. broadcast the transaction to the network
//
//     ps. unlock all available UTXO and send the remaining funds back to the source address helps us to avoid solving utxo selection problem
//         and we call it the UTXO aggregation approach


async fn inscription_sample() {
    let secp = Secp256k1::new();

    // P2WPKH private key here (in WIF format)
    let sk_wif = "cRz3eG99BvR8VnseYPsGYEiQ8oZCgeHJxKJ3yDXPYEyNKKZHkHdB";
    let private_key = PrivateKey::from_wif(sk_wif).unwrap();

    // Derive a public key
    let public_key = private_key.public_key(&secp);

    // Derive a P2WPKH address
    let address = Address::p2wpkh(&public_key, Network::Testnet).unwrap();
    
    println!("P2WPKH address: {}", address);

    // Create RPC client
    let rpc_url = "http://localhost:18332"; // Testnet RPC URL
    let rpc_auth = Auth::UserPass("rpcuser".to_string(), "rpcpassword".to_string());
    let rpc = Client::new(rpc_url, rpc_auth).unwrap();

    // Fetch UTXOs for the address
    let unspent_txs = rpc.list_unspent(Some(0), Some(9999999), Some(&[address.to_string()])).unwrap();

    // Prepare inputs
    let mut total_input_value = 0;
    let mut txins = vec![];
    for utxo in unspent_txs.iter() {
        let txid = bitcoin::Txid::from_str(&utxo.txid).unwrap();
        let vout = utxo.vout;
        let script_pubkey = address.script_pubkey();

        total_input_value += utxo.amount.as_sat();
        txins.push(TxIn {
            previous_output: bitcoin::OutPoint { txid, vout },
            script_sig: script_pubkey,
            sequence: 0xfffffffe,
            witness: vec![],
        });
    }

    // Create Inscription output using Taproot approach
    let inscription_data = b"Your Inscription Data Here".to_vec(); // Your inscription data

    let tap_script = Builder::new()
        .push_key(&public_key)
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(&inscription_data)
        .push_opcode(OP_ENDIF)
        .into_script();

    let tap_leaf_hash = TapLeafHash::from_script(&tap_script, bitcoin::util::taproot::LeafVersion::TapScript);
    let taproot_builder = TaprootBuilder::new().add_leaf(0, tap_leaf_hash).unwrap();
    let (taproot_output, _, _) = taproot_builder.finalize(&secp, public_key.key).unwrap();

    let inscription_output = TxOut {
        value: 1000, // Set appropriate value
        script_pubkey: taproot_output.to_v0_p2tr().unwrap(),
    };

    // Create P2WPKH change output to send the remaining funds back to the source address
    let change_output = TxOut {
        value: total_input_value - 1000 - 500, // Subtract the inscription value and fee
        script_pubkey: address.script_pubkey(),
    };

    // Create the transaction
    let tx = Transaction {
        version: 2,
        lock_time: 0,
        input: txins,
        output: vec![inscription_output, change_output],
    };

    // Sign the transaction
    let mut tx_to_sign = tx.clone();
    let mut sig_hash_cache = SigHashCache::new(&tx);
    for i in 0..tx.input.len() {
        let sig_hash = sig_hash_cache.signature_hash(i, &address.script_pubkey(), 1000, bitcoin::SigHashType::All);
        let message = bitcoin::secp256k1::Message::from_slice(&sig_hash[..]).unwrap();
        let sig = secp.sign(&message, &private_key.key);
        tx_to_sign.input[i].witness.push(sig.serialize_der().to_vec());
        tx_to_sign.input[i].witness.push(public_key.to_bytes());
    }

    // Broadcast the transaction
    let raw_tx = serialize(&tx_to_sign);
    let tx_hex = raw_tx.to_hex();
    rpc.send_raw_transaction(&raw_tx).unwrap();

    println!("Transaction broadcasted: {:?}", tx_hex);
}



#[tokio::main]
async fn main() {
    inscription_sample().await;
}