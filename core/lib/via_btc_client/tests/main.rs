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
mod segwit;
mod testnet_sample;

fn main() {
    testnet_sample::process_inscribe();
}
