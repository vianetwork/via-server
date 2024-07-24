use std::str::FromStr;

use bitcoin::key::Keypair;
use bitcoin::locktime::absolute;
use bitcoin::opcodes::{all, OP_FALSE};
use bitcoin::script::{Builder as ScriptBuilder, PushBytesBuf};
use bitcoin::secp256k1::{rand, Message, Secp256k1, SecretKey, Signing};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::taproot::TaprootBuilder;
use bitcoin::{
    transaction, Address, Amount, CompressedPublicKey, Network, OutPoint, PrivateKey, ScriptBuf,
    Sequence, Transaction, TxIn, TxOut, Txid, WPubkeyHash, Witness,
};
use bitcoincore_rpc::RawTx;

const UTXO_AMOUNT: Amount = Amount::from_sat(3_990);
const CHANGE_AMOUNT: Amount = Amount::from_sat(2_990); // 1000 sat fee.

fn senders_keys<C: Signing>(secp: &Secp256k1<C>) -> (SecretKey, WPubkeyHash, Keypair) {
    let private_key_wif = "cRz3eG99BvR8VnseYPsGYEiQ8oZCgeHJxKJ3yDXPYEyNKKZHkHdB";
    let private_key = PrivateKey::from_wif(private_key_wif).expect("Invalid WIF format");
    let sk = private_key.inner;

    let pk = bitcoin::PublicKey::new(sk.public_key(secp));
    let wpkh = pk.wpubkey_hash().expect("key is compressed");

    let compressed_pk = CompressedPublicKey::from_private_key(secp, &private_key).unwrap();
    let address = Address::p2wpkh(&compressed_pk, Network::Testnet);

    println!("wpkh: {}", wpkh);
    println!("address: {}", address);

    let keypair = Keypair::from_secret_key(secp, &sk);
    (sk, wpkh, keypair)
}

fn unspent_transaction_output(wpkh: &WPubkeyHash) -> (OutPoint, TxOut) {
    let script_pubkey = ScriptBuf::new_p2wpkh(wpkh);

    let txid = "e3ab971eca60617693a2395cc315eda75ec731bb24485e495768dd940a4e3a1a";
    let out_point = OutPoint {
        txid: Txid::from_str(&txid).unwrap(), // Obviously invalid.
        vout: 0,
    };

    let utxo = TxOut {
        value: UTXO_AMOUNT,
        script_pubkey,
    };

    (out_point, utxo)
}

#[allow(dead_code)]
pub fn process_inscribe() {
    let secp = Secp256k1::new();

    // Get a secret key we control and the pubkeyhash of the associated pubkey.
    // In a real application these would come from a stored secret.
    let (sk, wpkh, keypair) = senders_keys(&secp);
    let (internal_key, _parity) = keypair.x_only_public_key();

    // Get an unspent output that is locked to the key above that we control.
    // In a real application these would come from the chain.
    let (selected_out_point, selected_utxo) = unspent_transaction_output(&wpkh);

    // The input for the transaction we are constructing.
    let input = TxIn {
        previous_output: selected_out_point, // The dummy output we are spending.
        script_sig: ScriptBuf::default(),    // For a p2wpkh script_sig is empty.
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::default(), // Filled in after signing.
    };

    let serelized_pubkey = internal_key.serialize();
    let mut encoded_pubkey =
        PushBytesBuf::with_capacity(serelized_pubkey.len());
    encoded_pubkey.extend_from_slice(&serelized_pubkey).ok();


    // The inscription output with using Taproot approach:
    let taproot_script = ScriptBuilder::new()
        .push_slice(encoded_pubkey.as_push_bytes())
        .push_opcode(all::OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(all::OP_IF)
        .push_slice(b"***Hello From Via Inscriber***")
        .push_opcode(all::OP_ENDIF)
        .into_script();

    // Create a Taproot builder
    let mut builder = TaprootBuilder::new();
    builder = builder
        .add_leaf(0, taproot_script.clone())
        .expect("adding leaf should work");

    let taproot_spend_info = builder
        .finalize(&secp, internal_key)
        .expect("taproot finalize should work");

    // Create the Taproot output script
    let taproot_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), Network::Testnet);

    let inscription = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: taproot_address.script_pubkey(),
    };

    // The change output is locked to a key controlled by us.
    let change = TxOut {
        value: CHANGE_AMOUNT,
        script_pubkey: ScriptBuf::new_p2wpkh(&wpkh), // Change comes back to us.
    };

    // The transaction we want to sign and broadcast.
    let mut unsigned_commit_tx = Transaction {
        version: transaction::Version::TWO,  // Post BIP-68.
        lock_time: absolute::LockTime::ZERO, // Ignore the locktime.
        input: vec![input],                  // Input goes into index 0.
        output: vec![change, inscription],   // Outputs, order does not matter.
    };
    let input_index = 0;

    // Get the sighash to sign.
    let sighash_type = EcdsaSighashType::All;
    let mut sighasher = SighashCache::new(&mut unsigned_commit_tx);
    let sighash = sighasher
        .p2wpkh_signature_hash(
            input_index,
            &selected_utxo.script_pubkey,
            UTXO_AMOUNT,
            sighash_type,
        )
        .expect("failed to create sighash");

    // Sign the sighash using the secp256k1 library (exported by rust-bitcoin).
    let msg = Message::from(sighash);
    let signature = secp.sign_ecdsa(&msg, &sk);

    // Update the witness stack.
    let signature = bitcoin::ecdsa::Signature {
        signature,
        sighash_type,
    };
    let pk = sk.public_key(&secp);
    *sighasher.witness_mut(input_index).unwrap() = Witness::p2wpkh(&signature, &pk);

    // Get the signed transaction.
    let commit_tx = sighasher.into_transaction();

    // BOOM! Transaction signed and ready to broadcast.
    println!("{:#?}", commit_tx);

    println!("commit transaction: {:#?}", commit_tx.raw_hex().to_string());

    //**********************************************************************************/
    // start creating reveal transaction
    //**********************************************************************************/




    // // The input for the transaction we are constructing.
    // let input = TxIn {
    //     previous_output: dummy_out_point, // The dummy output we are spending.
    //     script_sig: ScriptBuf::default(), // For a p2tr script_sig is empty.
    //     sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    //     witness: Witness::default(), // Filled in after signing.
    // };


    // let reveal = TxIn {
    //     previous_output: selected_out_point, // The dummy output we are spending.
    //     script_sig: ScriptBuf::default(), // For a p2tr script_sig is empty.
    //     sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    //     witness: Witness::default(), // Filled in after signing.
    // };

    // // The change output is locked to a key controlled by us.
    // let change = TxOut {
    //     value: CHANGE_AMOUNT,
    //     script_pubkey: ScriptBuf::new_p2wpkh(&wpkh), // Change comes back to us.
    // };

    // // The transaction we want to sign and broadcast.
    // let mut unsigned_tx = Transaction {
    //     version: transaction::Version::TWO,  // Post BIP-68.
    //     lock_time: absolute::LockTime::ZERO, // Ignore the locktime.
    //     input: vec![input, reveal],                  // Input goes into index 0.
    //     output: vec![change],         // Outputs, order does not matter.
    // };
    // let input_index = 0;

    // // Get the sighash to sign.

    // let sighash_type = TapSighashType::Default;
    // let prevouts = vec![dummy_utxo];
    // let prevouts = Prevouts::All(&prevouts);

    // let mut sighasher = SighashCache::new(&mut unsigned_tx);
    // let sighash = sighasher
    //     .taproot_key_spend_signature_hash(input_index, &prevouts, sighash_type)
    //     .expect("failed to construct sighash");

    // // Sign the sighash using the secp256k1 library (exported by rust-bitcoin).
    // let tweaked: TweakedKeypair = keypair.tap_tweak(&secp, None);
    // let msg = Message::from_digest(sighash.to_byte_array());
    // let signature = secp.sign_schnorr(&msg, &tweaked.to_inner());

    // // Update the witness stack.
    // let signature = bitcoin::taproot::Signature {
    //     signature,
    //     sighash_type,
    // };
    // sighasher
    //     .witness_mut(input_index)
    //     .unwrap()
    //     .push(&signature.to_vec());

    // // Get the signed transaction.
    // let tx = sighasher.into_transaction();

    // // BOOM! Transaction signed and ready to broadcast.
    // println!("{:#?}", tx);


}
