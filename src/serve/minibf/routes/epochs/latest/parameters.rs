use axum::{extract::State, http::StatusCode, Json};

use crate::{ledger::pparams, serve::minibf::SharedState};
use itertools::Itertools as _;

use super::{
    CostModels, CostModelsRaw, CostParametersV1, CostParametersV2, CostParametersV3, ProtocolParams,
};

pub async fn route(State(state): State<SharedState>) -> Result<Json<ProtocolParams>, StatusCode> {
    let tip = state
        .ledger
        .cursor()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let updates = state
        .ledger
        .get_pparams(tip.as_ref().map(|p| p.0).unwrap_or_default())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updates: Vec<_> = updates
        .into_iter()
        .map(TryInto::try_into)
        .try_collect()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summary = pparams::fold_with_hacks(&state.genesis, &updates, tip.as_ref().unwrap().0);
    let era = summary.era_for_slot(tip.as_ref().unwrap().0);
    let mapper = pallas::interop::utxorpc::Mapper::new(state.ledger.clone());
    let mapped = mapper.map_pparams(era.pparams.clone());

    let pparams = ProtocolParams {
        epoch: era.start.epoch,
        min_fee_a: mapped.min_fee_coefficient,
        min_fee_b: mapped.min_fee_constant,
        max_block_size: mapped.max_block_body_size,
        max_tx_size: mapped.max_tx_size,
        max_block_header_size: mapped.max_block_header_size,
        key_deposit: mapped.stake_key_deposit.to_string(),
        pool_deposit: mapped.pool_deposit.to_string(),
        n_opt: mapped.desired_number_of_pools,
        protocol_major_ver: mapped.protocol_version.clone().unwrap().major as u64,
        protocol_minor_ver: mapped.protocol_version.clone().unwrap().minor as u64,
        min_pool_cost: mapped.min_pool_cost.to_string(),
        cost_models: mapped.cost_models.clone().map(|cost_models| CostModels {
            plutus_v1: cost_models
                .plutus_v1
                .map(|v1| CostParametersV1::from(&v1.values)),
            plutus_v2: cost_models
                .plutus_v2
                .map(|v2| CostParametersV2::from(&v2.values)),
            plutus_v3: cost_models
                .plutus_v3
                .map(|v3| CostParametersV3::from(&v3.values)),
        }),
        cost_models_raw: mapped.cost_models.clone().map(|cost_models| CostModelsRaw {
            plutus_v1: cost_models.plutus_v1.map(|v1| v1.values),
            plutus_v2: cost_models.plutus_v2.map(|v2| v2.values),
            plutus_v3: cost_models.plutus_v3.map(|v3| v3.values),
        }),
        price_mem: match &mapped.prices {
            Some(x) => x
                .memory
                .as_ref()
                .map(|x| x.numerator as f64 / x.denominator as f64),
            None => None,
        },
        price_step: match &mapped.prices {
            Some(x) => x
                .steps
                .as_ref()
                .map(|x| x.numerator as f64 / x.denominator as f64),
            None => None,
        },
        coins_per_utxo_size: Some(mapped.coins_per_utxo_byte.to_string()),
        coins_per_utxo_word: Some(mapped.coins_per_utxo_byte.to_string()),
        max_tx_ex_mem: mapped
            .max_execution_units_per_transaction
            .clone()
            .map(|units| units.memory.to_string()),
        max_tx_ex_steps: mapped
            .max_execution_units_per_transaction
            .clone()
            .map(|units| units.steps.to_string()),
        max_block_ex_mem: mapped
            .max_execution_units_per_block
            .clone()
            .map(|units| units.memory.to_string()),
        max_block_ex_steps: mapped
            .max_execution_units_per_block
            .clone()
            .map(|units| units.steps.to_string()),
        max_val_size: Some(mapped.max_value_size.to_string()),
        collateral_percent: Some(mapped.collateral_percentage),
        max_collateral_inputs: Some(mapped.max_collateral_inputs),
        min_fee_ref_script_cost_per_byte: mapped
            .min_fee_script_ref_cost_per_byte
            .as_ref()
            .map(|x| x.numerator as f64 / x.denominator as f64),
        e_max: state.genesis.shelley.protocol_params.e_max,
        a0: state.genesis.shelley.protocol_params.a0.numerator as f64
            / state.genesis.shelley.protocol_params.a0.denominator as f64,
        rho: mapped
            .monetary_expansion
            .as_ref()
            .map(|x| x.numerator as f64 / x.denominator as f64)
            .unwrap_or_default(),
        tau: mapped
            .treasury_expansion
            .as_ref()
            .map(|x| x.numerator as f64 / x.denominator as f64)
            .unwrap_or_default(),
        drep_deposit: Some(mapped.drep_deposit.to_string()),
        drep_activity: Some(mapped.drep_inactivity_period.to_string()),
        ..Default::default()
    };

    Ok(Json(pparams))
}
