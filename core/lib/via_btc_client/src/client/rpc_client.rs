use async_trait::async_trait;
use bitcoin::{Address, Block, BlockHash, OutPoint, Transaction, Txid};
pub use bitcoincore_rpc::Auth;
use bitcoincore_rpc::{
    bitcoincore_rpc_json::EstimateMode,
    json::{EstimateSmartFeeResult, ImportDescriptors, ScanTxOutRequest, Timestamp},
    Client, RpcApi,
};

use crate::{traits::BitcoinRpc, types::BitcoinRpcResult};


pub struct BitcoinRpcClient {
    client: Client,
}

#[allow(unused)]
impl BitcoinRpcClient {
    pub fn new(url: &str, auth: Auth) -> Result<Self, bitcoincore_rpc::Error> {
        let client = Client::new(url, auth)?;
        Ok(Self { client })
    }
}

#[allow(unused)]
#[async_trait]
impl BitcoinRpc for BitcoinRpcClient {
    async fn get_balance(&self, address: &Address) -> BitcoinRpcResult<u64> {
        // let descriptor = format!("addr({})", address);
        // let request = vec![ScanTxOutRequest::Single(descriptor)];

        // let result = self.client.scan_tx_out_set_blocking(&request)?;

        let result = self.client.list_unspent(
            Some(1), // minconf
            None, // maxconf
            Some(&[address]), // addresses
            None, // include_unsafe
            None, // query_options
        )?;

        let total_amount: u64 = result
            .into_iter()
            .map(|unspent| unspent.amount.to_sat())
            .sum();

        Ok(total_amount)
    }

    async fn send_raw_transaction(&self, tx_hex: &str) -> BitcoinRpcResult<Txid> {
        self.client
            .send_raw_transaction(tx_hex)
            .map_err(|e| e.into())
    }

    async fn import_address_to_node(&self, address: &Address) -> BitcoinRpcResult<()> {
        // check if the wallet with this name is exist in node or not. "watchonlywallet"
        // if not exist, create a new wallet with this name
        // import address to this wallet
        // call rescanblockchain on node

        let wallet_name = "watchonlywallet";
        let disable_private_keys = true;

        let wallets = self.client.list_wallets()?;

        let address_descriptor = format!("addr({})", address);
        let address_descriptor = self.client.get_descriptor_info(&address_descriptor)?;

        if !wallets.contains(&wallet_name.to_string()) {
            self.client.create_wallet(wallet_name, Some(disable_private_keys), None, None, None)?;
        } 

        let req = ImportDescriptors {
            descriptor: address_descriptor.descriptor,
            timestamp: Timestamp::Now,
            range: None,
            label: None,
            internal: None,
            active: None,
            next_index: None,
        };
        
        self.client.import_descriptors(req)?;

        Ok(())
    }

    async fn get_utxo_with_node_watch_only_wallet(&self, address: &Address) -> BitcoinRpcResult<Vec<OutPoint>> {
        let descriptor = format!("addr({})", address);

        let result = self.client.list_unspent(
            Some(1), // minconf
            None, // maxconf
            Some(&[address]), // addresses
            None, // include_unsafe
            None, // query_options
        )?;

        let unspent: Vec<OutPoint> = result
            .into_iter()
            .map(|unspent| OutPoint {
                txid: unspent.txid,
                vout: unspent.vout,
            })
            .collect();

        Ok(unspent)

    }

    async fn list_unspent(&self, address: &Address) -> BitcoinRpcResult<Vec<OutPoint>> {
        let descriptor = format!("addr({})", address);
        let request = vec![ScanTxOutRequest::Single(descriptor)];

        let result = self.client.scan_tx_out_set_blocking(&request)?;

        let unspent: Vec<OutPoint> = result
            .unspents
            .into_iter()
            .map(|unspent| OutPoint {
                txid: unspent.txid,
                vout: unspent.vout,
            })
            .collect();

        Ok(unspent)
    }

    async fn get_transaction(&self, txid: &Txid) -> BitcoinRpcResult<Transaction> {
        self.client
            .get_raw_transaction(txid, None)
            .map_err(|e| e.into())
    }

    async fn get_block_count(&self) -> BitcoinRpcResult<u64> {
        self.client.get_block_count().map_err(|e| e.into())
    }

    async fn get_block_by_height(&self, block_height: u128) -> BitcoinRpcResult<Block> {
        let block_hash = self.client.get_block_hash(block_height as u64)?;
        self.client.get_block(&block_hash).map_err(|e| e.into())
    }

    async fn get_block_by_hash(&self, block_hash: &BlockHash) -> BitcoinRpcResult<Block> {
        self.client.get_block(block_hash).map_err(|e| e.into())
    }

    async fn get_best_block_hash(&self) -> BitcoinRpcResult<bitcoin::BlockHash> {
        self.client.get_best_block_hash().map_err(|e| e.into())
    }

    async fn get_raw_transaction_info(
        &self,
        txid: &Txid,
    ) -> BitcoinRpcResult<bitcoincore_rpc::json::GetRawTransactionResult> {
        self.client
            .get_raw_transaction_info(txid, None)
            .map_err(|e| e.into())
    }

    async fn estimate_smart_fee(
        &self,
        conf_target: u16,
        estimate_mode: Option<EstimateMode>,
    ) -> BitcoinRpcResult<EstimateSmartFeeResult> {
        self.client
            .estimate_smart_fee(conf_target, estimate_mode)
            .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {}
