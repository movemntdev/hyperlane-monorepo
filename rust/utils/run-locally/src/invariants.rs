use std::path::Path;

use crate::config::Config;
use maplit::hashmap;

use crate::fetch_metric;
use crate::logging::log;
use crate::solana::solana_termination_invariants_met;

// This number should be even, so the messages can be split into two equal halves
// sent before and after the relayer spins up, to avoid rounding errors.
pub const APTOS_MESSAGES_EXPECTED: u32 = 20;
pub const SOL_MESSAGES_EXPECTED: u32 = 0; //20;
const SCRAPER_PORT: &str = "9092";

/// Use the metrics to check if the relayer queues are empty and the expected
/// number of messages have been sent.
pub fn termination_invariants_met(
    config: &Config,
    solana_cli_tools_path: &Path,
    solana_config_path: &Path,
) -> eyre::Result<bool> {
    let eth_messages_expected = 0; // (config.kathy_messages / 2) as u32 * 2; // no eth messages sent
    let total_messages_expected =
        eth_messages_expected + SOL_MESSAGES_EXPECTED + APTOS_MESSAGES_EXPECTED;

    let lengths = fetch_metric(SCRAPER_PORT, "hyperlane_submitter_queue_length", &hashmap! {})?;
    assert!(!lengths.is_empty(), "Could not find queue length metric");
    if lengths.iter().any(|n| *n != 0) {
        log!("Relayer queues not empty. Lengths: {:?}", lengths);
        return Ok(false);
    };

    // Also ensure the counter is as expected (total number of messages), summed
    // across all mailboxes.
    let msg_processed_count =
        fetch_metric(SCRAPER_PORT, "hyperlane_messages_processed_count", &hashmap! {})?
            .iter()
            .sum::<u32>();
    if msg_processed_count != total_messages_expected {
        log!(
            "Relayer has {} processed messages, expected {}",
            msg_processed_count,
            total_messages_expected
        );
        return Ok(false);
    }

    // expect number of dispatched messages sent to be equal to the total number of messages
    let dispatched_messages_scraped = fetch_metric(
        SCRAPER_PORT,
        "hyperlane_contract_sync_stored_events",
        &hashmap! {"data_type" => "dispatched_messages"},
    )
        .map_err(|err| {
            log!("error getting hyperlane_contract_sync_stored_events: {:?}", err);
            err
        })?
        .iter()
        .sum::<u32>();
    if dispatched_messages_scraped != total_messages_expected {
        log!(
            "Scraper has scraped {} dispatched messages, expected {}",
            dispatched_messages_scraped,
            total_messages_expected
        );
        return Ok(false);
    }

    let gas_payments_scraped = fetch_metric(
        SCRAPER_PORT,
        "hyperlane_contract_sync_stored_events",
        &hashmap! {"data_type" => "gas_payment"},
    )?
        .iter()
        .sum::<u32>();

    // number of gas payments equials number of messages sent
    let expected_gas_payments = total_messages_expected;
    if gas_payments_scraped != expected_gas_payments {
        log!(
            "Scraper has scraped {} gas payments, expected {}",
            gas_payments_scraped,
            expected_gas_payments
        );
        return Ok(false);
    }

    log!("Termination invariants have been meet");
    Ok(true)
}
