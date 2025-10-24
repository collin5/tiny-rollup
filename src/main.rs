use clap::Parser;
use jsonrpsee::server::{ServerBuilder, Server};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

use crate::{
    batch_processor::batch_processor::BatchProcessor, 
    rpc_server::server::{RollupRpcImpl, RollupRpcServer},
    sequencer::sequencer::Sequencer, 
    state_manager::state_manager::StateManager,
    transaction_processor::transaction_processor::TransactionProcessor,
};

mod batch_processor;
mod rpc_server;
mod sequencer;
mod state_manager;
mod transaction_processor;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "8899")]
    port: u16,

    #[arg(short, long, default_value = "./rollup_db")]
    db_path: String,

    #[arg(short, long)]
    solana_rpc: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize components
    let state_manager = Arc::new(StateManager::new(&args.db_path)?);
    let transaction_processor = Arc::new(TransactionProcessor::new(state_manager.clone()));
    let (sequencer, batch_receiver) = Sequencer::new(state_manager.clone());
    let sequencer = Arc::new(sequencer);

    // Start sequencer
    let sequencer_clone = sequencer.clone();
    tokio::spawn(async move {
        sequencer_clone.start_batching().await;
    });

    // Start batch processor
    let batch_processor = BatchProcessor::new(args.solana_rpc);
    tokio::spawn(async move {
        batch_processor.process_batches(batch_receiver).await;
    });

    // Start RPC Server
    let rpc_impl = RollupRpcImpl::new(state_manager, transaction_processor, sequencer);
    
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let middleware = tower::ServiceBuilder::new().layer(cors);

    let server = ServerBuilder::default()
        .set_http_middleware(middleware)
        .build(format!("0.0.0.0:{}", args.port))
        .await?;

    let handle = server.start(rpc_impl.into_rpc());

    println!("🚀 Rollup validator started on port {}", args.port);
    println!("Users can connect with: http://localhost:{}", args.port);

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    handle.stop()?;

    Ok(())
}

