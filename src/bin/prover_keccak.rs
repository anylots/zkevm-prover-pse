use env_logger::Env;

use halo2_proofs::{
    circuit::Value,
    dev::{MockProver, VerifyFailure},
    halo2curves::bn256::Fr,
    plonk::Circuit,
};
use zkevm_circuits::{
    evm_circuit::EvmCircuit,
    keccak_circuit::keccak_packed_multi::multi_keccak,
    super_circuit::SuperCircuit,
    util::{Challenges, SubCircuit},
    witness,
};

use bus_mapping::{
    circuit_input_builder::{keccak_inputs, BuilderClient, CircuitsParams},
    rpc::GethClient,
};

use ethers::providers::Http;
use std::{str::FromStr, sync::Once};

/// This command generates and prints the proofs to stdout.
/// Required environment variables:
/// - PROVERD_BLOCK_NUM - the block number to generate the proof for
/// - PROVERD_RPC_URL - a geth http rpc that supports the debug namespace
/// - PROVERD_PARAMS_PATH - a path to a file generated with the gen_params tool

#[tokio::main]
async fn main() {
    test_print_circuits_size().await;
}

const CIRCUITS_PARAMS: CircuitsParams = CircuitsParams {
    max_rws: 30000,
    max_copy_rows: 30000,
    max_txs: 20,
    max_calldata: 30000,
    max_inner_blocks: 64,
    max_bytecode: 30000,
    max_keccak_rows: 0,
    max_exp_steps: 1000,
    max_evm_rows: 0,
};

async fn test_print_circuits_size() {
    log_init();
    let block_num = 5;
    log::info!("test circuits size, block number: {}", block_num);
    let url = Http::from_str("http://127.0.0.1:8569").unwrap();
    let cli = GethClient::new(url);
    let cli = BuilderClient::new(cli, CIRCUITS_PARAMS).await.unwrap();
    let (builder, _) = cli.gen_inputs(block_num as u64).await.unwrap();

    if builder.block.txs.is_empty() {
        log::info!("skip empty block");
        return;
    }

    let block = witness::block_convert::<Fr>(&builder.block, &builder.code_db).unwrap();
    let evm_rows = EvmCircuit::get_num_rows_required(&block);
    let keccak_inputs = keccak_inputs(&builder.block, &builder.code_db).unwrap();

    let challenges = Challenges::mock(
        Value::known(block.randomness),
        Value::known(block.randomness),
        Value::known(block.randomness),
    );
    log::info!("===============start multi_keccak");

    let keccak_rows = multi_keccak(&keccak_inputs, challenges, None)
        .unwrap()
        .len();
    log::info!(
        "block number: {}, evm row {}, keccak row {}",
        block_num,
        evm_rows,
        keccak_rows
    );
}

/// Get the integration test [`GethClient`]
// pub fn get_client() -> GethClient<Http> {
//     let geth_client = GethClient::new(url);
// }

fn test_with<C: SubCircuit<Fr> + Circuit<Fr>>(block: &witness::Block<Fr>) -> MockProver<Fr> {
    let k = 22;
    let circuit = C::new_from_block(block);
    MockProver::<Fr>::run(k, &circuit, circuit.instance()).unwrap()
}

fn test_witness_block(block: &witness::Block<Fr>) -> Vec<VerifyFailure> {
    let prover = test_with::<SuperCircuit<Fr, 1, 2_000_000, 64, 0x1000>>(block);

    let result = prover.verify_par();
    result.err().unwrap_or_default()
}

static LOG_INIT: Once = Once::new();

/// Initialize log
pub fn log_init() {
    LOG_INIT.call_once(|| {
        env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    });
}
