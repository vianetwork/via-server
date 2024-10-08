use std::sync::Arc;

use anyhow::Context as _;
use zksync_concurrency::{ctx, error::Wrap as _, scope, time};
use zksync_config::configs::consensus::{ConsensusConfig, ConsensusSecrets};
use zksync_consensus_executor::{self as executor, attestation};
use zksync_consensus_roles::{attester, validator};
use zksync_consensus_storage::{BatchStore, BlockStore};

use crate::{
    config,
    storage::{ConnectionPool, InsertCertificateError, Store},
};

/// Task running a consensus validator for the main node.
/// Main node is currently the only leader of the consensus - i.e. it proposes all the
/// L2 blocks (generated by `Statekeeper`).
pub async fn run_main_node(
    ctx: &ctx::Ctx,
    cfg: ConsensusConfig,
    secrets: ConsensusSecrets,
    pool: ConnectionPool,
) -> anyhow::Result<()> {
    let validator_key = config::validator_key(&secrets)
        .context("validator_key")?
        .context("missing validator_key")?;

    let attester = config::attester_key(&secrets).context("attester_key")?;

    tracing::debug!(is_attester = attester.is_some(), "main node attester mode");

    scope::run!(&ctx, |ctx, s| async {
        if let Some(spec) = &cfg.genesis_spec {
            let spec = config::GenesisSpec::parse(spec).context("GenesisSpec::parse()")?;

            pool.connection(ctx)
                .await
                .wrap("connection()")?
                .adjust_genesis(ctx, &spec)
                .await
                .wrap("adjust_genesis()")?;
        }

        // The main node doesn't have a payload queue as it produces all the L2 blocks itself.
        let (store, runner) = Store::new(ctx, pool.clone(), None)
            .await
            .wrap("Store::new()")?;
        s.spawn_bg(runner.run(ctx));

        let (block_store, runner) = BlockStore::new(ctx, Box::new(store.clone()))
            .await
            .wrap("BlockStore::new()")?;
        s.spawn_bg(runner.run(ctx));

        let genesis = block_store.genesis().clone();
        anyhow::ensure!(
            genesis.leader_selection
                == validator::LeaderSelectionMode::Sticky(validator_key.public()),
            "unsupported leader selection mode - main node has to be the leader"
        );

        let (batch_store, runner) = BatchStore::new(ctx, Box::new(store.clone()))
            .await
            .wrap("BatchStore::new()")?;
        s.spawn_bg(runner.run(ctx));

        let attestation = Arc::new(attestation::Controller::new(attester));
        s.spawn_bg(run_attestation_updater(
            ctx,
            &pool,
            genesis,
            attestation.clone(),
        ));

        let executor = executor::Executor {
            config: config::executor(&cfg, &secrets)?,
            block_store,
            batch_store,
            validator: Some(executor::Validator {
                key: validator_key,
                replica_store: Box::new(store.clone()),
                payload_manager: Box::new(store.clone()),
            }),
            attestation,
        };

        tracing::info!("running the main node executor");
        executor.run(ctx).await
    })
    .await
}

/// Manages attestation state by configuring the
/// next batch to attest and storing the collected
/// certificates.
async fn run_attestation_updater(
    ctx: &ctx::Ctx,
    pool: &ConnectionPool,
    genesis: validator::Genesis,
    attestation: Arc<attestation::Controller>,
) -> anyhow::Result<()> {
    const POLL_INTERVAL: time::Duration = time::Duration::seconds(5);
    let res = async {
        let Some(committee) = &genesis.attesters else {
            return Ok(());
        };
        let committee = Arc::new(committee.clone());
        loop {
            // After regenesis it might happen that the batch number for the first block
            // is not immediately known (the first block was not produced yet),
            // therefore we need to wait for it.
            let status = loop {
                match pool
                    .connection(ctx)
                    .await
                    .wrap("connection()")?
                    .attestation_status(ctx)
                    .await
                    .wrap("attestation_status()")?
                {
                    Some(status) => break status,
                    None => ctx.sleep(POLL_INTERVAL).await?,
                }
            };
            tracing::info!(
                "waiting for hash of batch {:?}",
                status.next_batch_to_attest
            );
            let hash = pool
                .wait_for_batch_hash(ctx, status.next_batch_to_attest)
                .await?;
            tracing::info!(
                "attesting batch {:?} with hash {hash:?}",
                status.next_batch_to_attest
            );
            attestation
                .start_attestation(Arc::new(attestation::Info {
                    batch_to_attest: attester::Batch {
                        hash,
                        number: status.next_batch_to_attest,
                        genesis: status.genesis,
                    },
                    committee: committee.clone(),
                }))
                .await
                .context("start_attestation()")?;
            // Main node is the only node which can update the global AttestationStatus,
            // therefore we can synchronously wait for the certificate.
            let qc = attestation
                .wait_for_cert(ctx, status.next_batch_to_attest)
                .await?
                .context("attestation config has changed unexpectedly")?;
            tracing::info!(
                "collected certificate for batch {:?}",
                status.next_batch_to_attest
            );
            pool.connection(ctx)
                .await
                .wrap("connection()")?
                .insert_batch_certificate(ctx, &qc)
                .await
                .map_err(|err| match err {
                    InsertCertificateError::Canceled(err) => ctx::Error::Canceled(err),
                    InsertCertificateError::Inner(err) => ctx::Error::Internal(err.into()),
                })?;
        }
    }
    .await;
    match res {
        Ok(()) | Err(ctx::Error::Canceled(_)) => Ok(()),
        Err(ctx::Error::Internal(err)) => Err(err),
    }
}
