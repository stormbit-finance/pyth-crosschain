use {
    crate::{
        api::{get_register_uri, ChainId},
        chain::ethereum::{ProviderInfo, SignablePythContract},
        command::register_provider::register_provider_from_config,
        config::{Config, EthereumConfig, SetupProviderOptions},
    },
    anyhow::{anyhow, Result},
    ethers::{
        abi::Bytes as AbiBytes,
        signers::{LocalWallet, Signer},
        types::{Address, Bytes},
    },
    futures::future::join_all,
    std::sync::Arc,
    tokio::spawn,
    tracing::Instrument,
};

/// Setup provider for all the chains.
pub async fn setup_provider(opts: &SetupProviderOptions) -> Result<()> {
    let config = Config::load(&opts.config.config)?;
    let setup_tasks = config
        .chains
        .clone()
        .into_iter()
        .map(|(chain_id, chain_config)| {
            let config = config.clone();
            spawn(async move {
                (
                    setup_chain_provider(&config, &chain_id, &chain_config).await,
                    chain_id,
                )
            })
        })
        .collect::<Vec<_>>();
    let join_results = join_all(setup_tasks).await;
    let mut all_ok = true;
    for join_result in join_results {
        let (setup_result, chain_id) = join_result?;
        match setup_result {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to setup {} {}", chain_id, e);
                all_ok = false;
            }
        }
    }

    match all_ok {
        true => Ok(()),
        false => Err(anyhow!("Failed to setup provider for all chains")),
    }
}

/// Setup provider for a single chain.
/// 1. Register if there was no previous registration.
/// 4. Update provider fee if there is a mismatch with the fee set on contract.
/// 5. Update provider uri if there is a mismatch with the uri set on contract.
#[tracing::instrument(name = "setup_chain_provider", skip_all, fields(chain_id = chain_id))]
async fn setup_chain_provider(
    config: &Config,
    chain_id: &ChainId,
    chain_config: &EthereumConfig,
) -> Result<()> {
    tracing::info!("Setting up provider for chain: {0}", chain_id);
    let provider_config = &config.provider;
    let private_key = provider_config.private_key.load()?.ok_or(anyhow!(
        "Please specify a provider private key in the config file."
    ))?;
    let provider_address = private_key.clone().parse::<LocalWallet>()?.address();
    // Initialize a Provider to interface with the EVM contract.
    let contract = Arc::new(SignablePythContract::from_config(chain_config, &private_key).await?);

    tracing::info!("Fetching provider info");
    let provider_info = contract.get_provider_info(provider_address).call().await?;
    tracing::info!("Provider info: {:?}", provider_info);

    tracing::info!("Registering");
    register_provider_from_config(provider_config, chain_id, chain_config)
        .await
        .map_err(|e| anyhow!("Chain: {} - Failed to register provider: {}", &chain_id, e))?;
    tracing::info!("Registered");

    let provider_info = contract.get_provider_info(provider_address).call().await?;

    sync_fee(&contract, &provider_info, chain_config.fee)
        .in_current_span()
        .await?;

    let uri = get_register_uri(&provider_config.uri, chain_id)?;
    sync_uri(&contract, &provider_info, uri)
        .in_current_span()
        .await?;

    sync_fee_manager(
        &contract,
        &provider_info,
        provider_config.fee_manager.unwrap_or(Address::zero()),
    )
    .in_current_span()
    .await?;

    sync_max_num_hashes(
        &contract,
        &provider_info,
        chain_config.max_num_hashes.unwrap_or(0),
    )
    .in_current_span()
    .await?;

    Ok(())
}

async fn sync_uri(
    contract: &Arc<SignablePythContract>,
    provider_info: &ProviderInfo,
    uri: String,
) -> Result<()> {
    let uri_as_bytes: Bytes = AbiBytes::from(uri.as_str()).into();
    if provider_info.uri != uri_as_bytes {
        tracing::info!("Updating provider uri to {}", uri);
        if let Some(receipt) = contract
            .set_provider_uri(uri_as_bytes)
            .send()
            .await?
            .await?
        {
            tracing::info!("Updated provider uri: {:?}", receipt);
        }
    }
    Ok(())
}

async fn sync_fee(
    contract: &Arc<SignablePythContract>,
    provider_info: &ProviderInfo,
    provider_fee: u128,
) -> Result<()> {
    if provider_info.fee_in_wei != provider_fee {
        tracing::info!("Updating provider fee {}", provider_fee);
        if let Some(r) = contract
            .set_provider_fee(provider_fee)
            .send()
            .await?
            .await?
        {
            tracing::info!("Updated provider fee: {:?}", r);
        }
    }
    Ok(())
}

async fn sync_fee_manager(
    contract: &Arc<SignablePythContract>,
    provider_info: &ProviderInfo,
    fee_manager: Address,
) -> Result<()> {
    if provider_info.fee_manager != fee_manager {
        tracing::info!("Updating provider fee manager to {:?}", fee_manager);
        if let Some(receipt) = contract.set_fee_manager(fee_manager).send().await?.await? {
            tracing::info!("Updated provider fee manager: {:?}", receipt);
        }
    }
    Ok(())
}

async fn sync_max_num_hashes(
    contract: &Arc<SignablePythContract>,
    provider_info: &ProviderInfo,
    max_num_hashes: u32,
) -> Result<()> {
    if provider_info.max_num_hashes != max_num_hashes {
        tracing::info!("Updating provider max num hashes to {:?}", max_num_hashes);
        if let Some(receipt) = contract
            .set_max_num_hashes(max_num_hashes)
            .send()
            .await?
            .await?
        {
            tracing::info!("Updated provider max num hashes to : {:?}", receipt);
        }
    }
    Ok(())
}
