use std::{future::Future, time::Duration};

use anyhow::Context;
use chrono::{NaiveDateTime, Utc};
use rand::Rng;
use tokio::sync::watch;
use zksync_config::DADispatcherConfig;
use zksync_da_client::{
    types::{DAError, IsTransient},
    DataAvailabilityClient,
};
use zksync_dal::{ConnectionPool, Core, CoreDal};
use zksync_types::L1BatchNumber;

use crate::metrics::METRICS;

#[derive(Debug)]
pub struct DataAvailabilityDispatcher {
    client: Box<dyn DataAvailabilityClient>,
    pool: ConnectionPool<Core>,
    config: DADispatcherConfig,
}

impl DataAvailabilityDispatcher {
    pub fn new(
        pool: ConnectionPool<Core>,
        config: DADispatcherConfig,
        client: Box<dyn DataAvailabilityClient>,
    ) -> Self {
        Self {
            pool,
            config,
            client,
        }
    }

    pub async fn run(self, stop_receiver: watch::Receiver<bool>) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        loop {
            if *stop_receiver.borrow() {
                tracing::info!("Stop signal received, da_dispatcher is shutting down");
                break;
            }

            if let Err(err) = self.dispatch(&pool).await {
                tracing::warn!("dispatch error {err:?}");
            }

            if let Err(err) = self.poll_for_inclusion(&pool).await {
                tracing::warn!("poll_for_inclusion error {err:?}");
            }

            tokio::time::sleep(self.config.polling_interval()).await;
        }
        Ok(())
    }

    /// Dispatches the blobs to the data availability layer, and saves the blob_id in the database.
    async fn dispatch(&self, pool: &ConnectionPool<Core>) -> anyhow::Result<()> {
        let mut conn = pool.connection_tagged("da_dispatcher").await?;
        let batches = conn
            .data_availability_dal()
            .get_ready_for_da_dispatch_l1_batches(self.config.query_rows_limit() as usize)
            .await?;
        drop(conn);

        for batch in batches {
            let dispatch_latency = METRICS.blob_dispatch_latency.start();
            let dispatch_response = retry(self.config.max_retries(), batch.l1_batch_number, || {
                self.client
                    .dispatch_blob(batch.l1_batch_number.0, batch.pubdata.clone())
            })
            .await
            .with_context(|| {
                format!(
                    "failed to dispatch a blob with batch_number: {}, pubdata_len: {}",
                    batch.l1_batch_number,
                    batch.pubdata.len()
                )
            })?;
            let dispatch_latency_duration = dispatch_latency.observe();

            let sent_at =
                NaiveDateTime::from_timestamp_millis(Utc::now().timestamp_millis()).unwrap();

            let mut conn = pool.connection_tagged("da_dispatcher").await?;
            conn.data_availability_dal()
                .insert_l1_batch_da(
                    batch.l1_batch_number,
                    dispatch_response.blob_id.as_str(),
                    sent_at,
                )
                .await?;
            drop(conn);

            METRICS
                .last_dispatched_l1_batch
                .set(batch.l1_batch_number.0 as usize);
            METRICS.blob_size.observe(batch.pubdata.len());
            tracing::info!(
                "Dispatched a DA for batch_number: {}, pubdata_size: {}, dispatch_latency ms: {}",
                batch.l1_batch_number,
                batch.pubdata.len(),
                dispatch_latency_duration.as_millis()
            );
        }

        Ok(())
    }

    /// Polls the data availability layer for inclusion data, and saves it in the database.
    async fn poll_for_inclusion(&self, pool: &ConnectionPool<Core>) -> anyhow::Result<()> {
        let mut conn = pool.connection_tagged("da_dispatcher").await?;
        if let Some(blob_info) = conn
            .data_availability_dal()
            .get_first_da_blob_awaiting_inclusion()
            .await?
        {
            drop(conn);
            let inclusion_data = self
                .client
                .get_inclusion_data(blob_info.blob_id.clone())
                .await
                .with_context(|| {
                    format!(
                        "failed to get inclusion data for blob_id: {}, batch_number: {}",
                        blob_info.blob_id, blob_info.l1_batch_number
                    )
                })?;

            let mut conn = pool.connection_tagged("da_dispatcher").await?;
            if let Some(inclusion_data) = inclusion_data {
                conn.data_availability_dal()
                    .save_l1_batch_inclusion_data(
                        L1BatchNumber(blob_info.l1_batch_number.0),
                        inclusion_data.data.as_slice(),
                    )
                    .await?;
                drop(conn);

                let inclusion_latency = Utc::now().signed_duration_since(blob_info.sent_at);
                METRICS
                    .inclusion_latency
                    .observe(inclusion_latency.to_std()?);
                METRICS
                    .last_included_l1_batch
                    .set(blob_info.l1_batch_number.0 as usize);

                tracing::info!(
                    "Received an inclusion data for a batch_number: {}, inclusion_latency_seconds: {}",
                    blob_info.l1_batch_number, inclusion_latency.num_seconds()
                );
            }
        }

        Ok(())
    }
}

async fn retry<T, Fut, F>(
    max_retries: u16,
    batch_number: L1BatchNumber,
    mut f: F,
) -> Result<T, DAError>
where
    Fut: Future<Output = Result<T, DAError>>,
    F: FnMut() -> Fut,
{
    let mut retries = 1;
    let mut backoff_secs = 1;
    loop {
        match f().await {
            Ok(result) => {
                METRICS.dispatch_call_retries.observe(retries as usize);
                return Ok(result);
            }
            Err(err) => {
                if !err.is_transient() || retries > max_retries {
                    return Err(err);
                }

                retries += 1;
                let sleep_duration = Duration::from_secs(backoff_secs)
                    .mul_f32(rand::thread_rng().gen_range(0.8..1.2));
                tracing::warn!(%err, "Failed DA dispatch request {retries}/{max_retries} for batch {batch_number}, retrying in {} milliseconds.", sleep_duration.as_millis());
                tokio::time::sleep(sleep_duration).await;
                backoff_secs = (backoff_secs * 2).min(128); // cap the back-off at 128 seconds
            }
        }
    }
}