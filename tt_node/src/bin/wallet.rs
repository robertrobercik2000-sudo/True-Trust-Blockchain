//! TT Wallet Binary Entry Point

fn main() -> anyhow::Result<()> {
    tt_node::wallet::wallet_cli::run_cli()
}
