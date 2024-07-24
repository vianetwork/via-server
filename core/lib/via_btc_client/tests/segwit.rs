use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::key::Keypair;
use bitcoin::locktime::absolute;
use bitcoin::opcodes::{all, OP_FALSE};
use bitcoin::script::Builder as ScriptBuilder;
use bitcoin::secp256k1::{rand, Message, Secp256k1, SecretKey, Signing};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::taproot::TaprootBuilder;
use bitcoin::{
    transaction, Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, WPubkeyHash, Witness,
};
use bitcoincore_rpc::RawTx;

const DUMMY_UTXO_AMOUNT: Amount = Amount::from_sat(20_000_000);
const SPEND_AMOUNT: Amount = Amount::from_sat(5_000_000);
const CHANGE_AMOUNT: Amount = Amount::from_sat(14_999_000); // 1000 sat fee.

fn senders_keys<C: Signing>(secp: &Secp256k1<C>) -> (SecretKey, WPubkeyHash, Keypair) {
    let sk = SecretKey::new(&mut rand::thread_rng());
    let pk = bitcoin::PublicKey::new(sk.public_key(secp));
    let wpkh = pk.wpubkey_hash().expect("key is compressed");

    let keypair = Keypair::from_secret_key(secp, &sk);
    (sk, wpkh, keypair)
}

fn receivers_address() -> Address {
    Address::from_str("bc1q7cyrfmck2ffu2ud3rn5l5a8yv6f0chkp0zpemf")
        .expect("a valid address")
        .require_network(Network::Bitcoin)
        .expect("valid address for mainnet")
}

fn dummy_unspent_transaction_output(wpkh: &WPubkeyHash) -> (OutPoint, TxOut) {
    let script_pubkey = ScriptBuf::new_p2wpkh(wpkh);

    let out_point = OutPoint {
        txid: Txid::all_zeros(), // Obviously invalid.
        vout: 0,
    };

    let utxo = TxOut {
        value: DUMMY_UTXO_AMOUNT,
        script_pubkey,
    };

    (out_point, utxo)
}

#[allow(dead_code)]
pub fn process_segwit() {
    let secp = Secp256k1::new();

    // Get a secret key we control and the pubkeyhash of the associated pubkey.
    // In a real application these would come from a stored secret.
    let (sk, wpkh, keypair) = senders_keys(&secp);
    let (internal_key, _parity) = keypair.x_only_public_key();

    // Get an address to send to.
    let address = receivers_address();

    // Get an unspent output that is locked to the key above that we control.
    // In a real application these would come from the chain.
    let (dummy_out_point, dummy_utxo) = dummy_unspent_transaction_output(&wpkh);

    // The input for the transaction we are constructing.
    let input = TxIn {
        previous_output: dummy_out_point, // The dummy output we are spending.
        script_sig: ScriptBuf::default(), // For a p2wpkh script_sig is empty.
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::default(), // Filled in after signing.
    };

    // The spend output is locked to a key controlled by the receiver.
    let spend = TxOut {
        value: SPEND_AMOUNT,
        script_pubkey: address.script_pubkey(),
    };

    // The inscription output with using Taproot approach:
    let taproot_script = ScriptBuilder::new()
        .push_opcode(OP_FALSE)
        .push_opcode(all::OP_IF)
        .push_slice(b"Hello From Via Inscriber")
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
    let taproot_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), Network::Bitcoin);

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
    let mut unsigned_tx = Transaction {
        version: transaction::Version::TWO,       // Post BIP-68.
        lock_time: absolute::LockTime::ZERO,      // Ignore the locktime.
        input: vec![input],                       // Input goes into index 0.
        output: vec![spend, change, inscription], // Outputs, order does not matter.
    };
    let input_index = 0;

    // Get the sighash to sign.
    let sighash_type = EcdsaSighashType::All;
    let mut sighasher = SighashCache::new(&mut unsigned_tx);
    let sighash = sighasher
        .p2wpkh_signature_hash(
            input_index,
            &dummy_utxo.script_pubkey,
            DUMMY_UTXO_AMOUNT,
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
    let tx = sighasher.into_transaction();

    // BOOM! Transaction signed and ready to broadcast.
    println!("{:#?}", tx);

    println!("{:#?}", tx.raw_hex().to_string());
}
