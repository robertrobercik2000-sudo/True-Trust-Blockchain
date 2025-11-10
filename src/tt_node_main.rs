#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;

use tt_priv_cli::crypto_kmac_consensus::kmac256_hash;
use tt_priv_cli::pot::{NodeId, PotParams, TrustParams, ONE_Q, Q};
use tt_priv_cli::pot_node::{GenesisValidator, NodeError, PotNode, PotNodeConfig};

#[derive(Parser, Debug)]
#[command(name = "tt_node", version, author)]
#[command(about = "True-Trust Proof-of-Trust production node")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,

    /// Path to JSON config (default: ./tt_node.json)
    #[arg(long, global = true, default_value = "tt_node.json")]
    config: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the node simulation for a number of slots
    Run {
        /// Number of slots to simulate
        #[arg(long, default_value_t = 32)]
        slots: u64,
    },
    /// Print the active validator snapshot
    Snapshot,
}

#[derive(Deserialize)]
struct FileConfig {
    node_id: String,
    slot_duration_secs: u64,
    epoch_length: u64,
    lambda: f64,
    min_bond: u64,
    slash_noreveal_bps: u32,
    equivocation_bps: u32,
    genesis_beacon: String,
    trust: TrustSection,
    validators: Vec<FileValidator>,
}

#[derive(Deserialize)]
struct TrustSection {
    alpha: f64,
    beta: f64,
    init: f64,
}

#[derive(Deserialize)]
struct FileValidator {
    id: String,
    stake: u64,
    active: bool,
    trust_override: Option<f64>,
}

impl FileConfig {
    fn parse(self) -> Result<(PotNodeConfig, Vec<GenesisValidator>, [u8; 32])> {
        let node_id = parse_hex_32(&self.node_id).context("invalid node_id hex")?;
        let genesis_beacon =
            parse_hex_32(&self.genesis_beacon).context("invalid genesis_beacon hex")?;

        let trust = TrustParams {
            alpha_q: q_from_float(self.trust.alpha)?,
            beta_q: q_from_float(self.trust.beta)?,
            init_q: q_from_float(self.trust.init)?,
        };

        let params = PotParams {
            trust,
            lambda_q: q_from_factor(self.lambda)?,
            min_bond: self.min_bond,
            slash_noreveal_bps: self.slash_noreveal_bps,
        };

        let config = PotNodeConfig {
            node_id,
            slot_duration: Duration::from_secs(self.slot_duration_secs.max(1)),
            epoch_length: self.epoch_length.max(1),
            params,
            equivocation_penalty_bps: self.equivocation_bps,
        };

        let validators = self
            .validators
            .into_iter()
            .map(|v| {
                Ok(GenesisValidator {
                    who: parse_hex_32(&v.id).context("invalid validator id")?,
                    stake: v.stake,
                    active: v.active,
                    trust_override: v.trust_override.map(q_from_float).transpose()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((config, validators, genesis_beacon))
    }
}

fn parse_hex_32(s: &str) -> Result<NodeId> {
    let bytes = hex::decode(s).context("hex decode failed")?;
    if bytes.len() != 32 {
        return Err(anyhow!("expected 32-byte hex value"));
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Ok(id)
}

fn q_from_float(value: f64) -> Result<Q> {
    if !(0.0..=1.0).contains(&value) {
        return Err(anyhow!("Q values must be between 0 and 1"));
    }
    let scaled = (value * (ONE_Q as f64)).round();
    Ok(scaled.clamp(0.0, ONE_Q as f64) as u64)
}

fn q_from_factor(value: f64) -> Result<Q> {
    if value <= 0.0 {
        return Err(anyhow!("lambda must be positive"));
    }
    let scaled = value * (ONE_Q as f64);
    let max = u64::MAX as f64;
    Ok(scaled.min(max).round() as u64)
}

fn load_config(path: &PathBuf) -> Result<(PotNodeConfig, Vec<GenesisValidator>, [u8; 32])> {
    let data = fs::read_to_string(path).with_context(|| format!("reading config {:?}", path))?;
    let cfg: FileConfig = serde_json::from_str(&data).context("parsing JSON config")?;
    cfg.parse()
}

fn simulate(mut node: PotNode, slots: u64) {
    println!("🚀 Starting Proof-of-Trust node");
    println!("   Node ID: {}", hex::encode(node.config().node_id));
    println!("   Epoch length: {} slots", node.config().epoch_length);

    let mut slots_in_epoch = 0u64;
    for slot in 0..slots {
        let epoch = node.snapshot().epoch;
        let order: Vec<NodeId> = node.snapshot().order.clone();
        let mut produced = None;
        for who in &order {
            if let Some(wit) = node.witness_for(who) {
                let header = kmac256_hash(
                    b"TT.node.header",
                    &[&epoch.to_le_bytes(), &slot.to_le_bytes(), who],
                );
                match node.process_proposal(epoch, slot, &wit, header) {
                    Ok(decision) => {
                        if decision.equivocation_slashed {
                            println!(
                                "⚖️  Slot {}: {:?} equivocated (slashed).",
                                slot,
                                hex::encode(who)
                            );
                        } else {
                            produced = Some(decision);
                            break;
                        }
                    }
                    Err(NodeError::NotEligible) => continue,
                    Err(e) => {
                        println!("⚠️  Slot {} proposal rejected: {}", slot, e);
                    }
                }
            }
        }

        if let Some(decision) = produced {
            println!(
                "✅ Slot {} elected {:?} (weight={} trust={:.4})",
                slot,
                hex::encode(decision.proposer),
                decision.weight,
                (decision.trust_after as f64) / (ONE_Q as f64),
            );
        } else {
            println!("⏳ Slot {} had no eligible proposer", slot);
        }

        slots_in_epoch += 1;
        if slots_in_epoch >= node.config().epoch_length {
            let beacon = node.finalize_epoch();
            println!(
                "🔄 Finalized epoch {}. New beacon = {}",
                epoch,
                hex::encode(beacon)
            );
            slots_in_epoch = 0;
        }
    }
}

fn print_snapshot(node: &PotNode) {
    println!("📊 Active snapshot (epoch {})", node.snapshot().epoch);
    for who in &node.snapshot().order {
        let stake_q = node.snapshot().stake_q_of(who);
        let trust_q = node.snapshot().trust_q_of(who);
        println!(
            " - {} | stake_q={:.4} trust_q={:.4}",
            hex::encode(who),
            (stake_q as f64) / (ONE_Q as f64),
            (trust_q as f64) / (ONE_Q as f64)
        );
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (config, validators, beacon) = load_config(&cli.config)?;
    let node = PotNode::new(config, validators, beacon);

    match cli.cmd {
        Command::Run { slots } => simulate(node, slots),
        Command::Snapshot => print_snapshot(&node),
    }

    Ok(())
}
