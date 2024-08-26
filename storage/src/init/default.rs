use crate::{Error, Result};
use rand::Rng;
use solana_entry::entry::create_ticks;
use solana_ledger::{
    blockstore::Blockstore,
    blockstore_options::{AccessType, BlockstoreOptions},
    genesis_utils::GenesisConfigInfo,
    shred::{ProcessShredsStats, ReedSolomonCache, Shredder},
};
use solana_runtime::genesis_utils::create_genesis_config_with_leader_ex;
use solana_sdk::{
    fee_calculator::FeeRateGovernor,
    genesis_config::{ClusterType, GenesisConfig},
    hash::Hash,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
};
use std::path::Path;

pub(crate) const DEFAULT_VALIDATOR_LAMPORTS: u64 = 10_000_000;
pub(crate) const DEFAULT_MINT_LAMPORTS: u64 = 1_000_000_000;
pub(crate) const DEFAULT_STAKE_LAMPORTS: u64 = 50_000_000;

pub(crate) fn default_genesis_config(ledger_path: &Path) -> Result<(GenesisConfigInfo, Keypair)> {
    let validator_key = Keypair::new();
    let mint_keypair = Keypair::new();
    let voting_keypair = Keypair::new();
    let genesis_config = create_genesis_config_with_leader_ex(
        DEFAULT_MINT_LAMPORTS,
        &mint_keypair.pubkey(),
        &validator_key.pubkey(),
        &voting_keypair.pubkey(),
        &solana_sdk::pubkey::new_rand(),
        DEFAULT_STAKE_LAMPORTS,
        DEFAULT_VALIDATOR_LAMPORTS,
        FeeRateGovernor::new(0, 0), // most tests can't handle transaction fees
        Rent::free(),               // most tests don't expect rent
        ClusterType::Development,
        vec![],
    );
    init_block_store(ledger_path, &genesis_config)?;

    Ok((
        GenesisConfigInfo {
            genesis_config,
            mint_keypair,
            voting_keypair,
            validator_pubkey: validator_key.pubkey(),
        },
        validator_key,
    ))
}

fn init_block_store(ledger_path: &Path, genesis_config: &GenesisConfig) -> Result<()> {
    genesis_config
        .write(ledger_path)
        .map_err(|e| Error::InitCommon(format!("failed to save genesis config: {e}")))?;

    let blockstore = Blockstore::open_with_options(
        ledger_path,
        BlockstoreOptions {
            access_type: AccessType::Primary,
            recovery_mode: None,
            enforce_ulimit_nofile: false,
            column_options: Default::default(),
        },
    )?;
    let ticks_per_slot = genesis_config.ticks_per_slot;
    let hashes_per_tick = genesis_config.poh_config.hashes_per_tick.unwrap_or(0);
    let entries = create_ticks(ticks_per_slot, hashes_per_tick, genesis_config.hash());
    let last_hash = entries.last().unwrap().hash;
    let version = solana_sdk::shred_version::version_from_hash(&last_hash);

    let shredder = Shredder::new(0, 0, 0, version).unwrap();
    let (shreds, _) = shredder.entries_to_shreds(
        &Keypair::new(),
        &entries,
        true, // is_last_in_slot
        // chained_merkle_root
        Some(Hash::new_from_array(rand::thread_rng().gen())),
        0,    // next_shred_index
        0,    // next_code_index
        true, // merkle_variant
        &ReedSolomonCache::default(),
        &mut ProcessShredsStats::default(),
    );
    assert!(shreds.last().unwrap().last_in_slot());

    blockstore.insert_shreds(shreds, None, false)?;
    blockstore.set_roots(std::iter::once(&0))?;
    // Explicitly close the blockstore before we create the archived genesis file
    drop(blockstore);

    Ok(())
}