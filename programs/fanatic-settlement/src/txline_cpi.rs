/// TxLINE CPI Integration Module
///
/// This module handles Cross-Program Invocation (CPI) into the TxLINE protocol
/// for trustless match outcome verification via Merkle proofs.
///
/// TxLINE's on-chain program exposes a `validate_stat` instruction that accepts:
/// - A match identifier
/// - A stat key (e.g., "match.first_goal_scorer")
/// - A Merkle proof (series of sibling hashes)
/// - A leaf value (the claimed outcome)
/// - A Merkle root (anchor point on Solana)
///
/// The instruction verifies the Merkle proof and returns the validated stat value.
/// Our program uses this CPI to resolve prediction markets without trusting any
/// single oracle or authority.
use anchor_lang::prelude::*;

/// TxLINE program ID — this is configured per-deployment via PlatformState
/// During the hackathon, TxLINE provides a canonical program ID for DevNet.

/// Account structure for TxLINE's validate_stat instruction.
/// These accounts are passed through CPI to TxLINE's on-chain program.
#[derive(Accounts)]
pub struct TxlineValidateStat<'info> {
    /// The OracleProof account on our side, storing the proof data
    /// CHECK: This account is verified within our program logic
    pub oracle_proof: AccountInfo<'info>,

    /// TxLINE's on-chain state account (stores Merkle roots)
    /// CHECK: TxLINE program validates this account
    pub txline_state: AccountInfo<'info>,

    /// Any additional accounts TxLINE requires for this validation
    /// CHECK: Passed through to TxLINE program
    pub txline_authority: AccountInfo<'info>,
}

/// Data structure passed to TxLINE's validate_stat instruction
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TxlineValidateStatArgs {
    /// TxLINE match identifier
    pub match_id: String,
    /// Stat key to validate (e.g., "match.first_goal_scorer")
    pub stat_key: String,
    /// The leaf value being claimed
    pub value: String,
    /// Merkle proof sibling hashes
    pub proof: Vec<[u8; 32]>,
    /// Merkle root to verify against
    pub merkle_root: [u8; 32],
}

/// Perform CPI into TxLINE's validate_stat instruction.
///
/// This function constructs the instruction data and account metas,
/// then invokes TxLINE's program via CPI. If the Merkle proof is valid,
/// TxLINE returns success. If invalid, the CPI fails and our instruction
/// reverts.
///
/// # Arguments
/// * `program_id` - The TxLINE program ID (from PlatformState)
/// * `ctx` - Accounts context for the CPI
/// * `args` - The validation arguments (match_id, stat_key, value, proof, root)
///
/// # Returns
/// * `Result<()>` - Ok if proof is valid, Err if invalid
pub fn cpi_validate_stat<'info>(
    txline_program_id: &Pubkey,
    accounts: &TxlineValidateStat<'info>,
    args: &TxlineValidateStatArgs,
) -> Result<()> {
    // Build the instruction data for TxLINE's validate_stat
    // TxLINE expects: [instruction_discriminator (8 bytes)] + [serialized args]
    let discriminator = anchor_lang::solana_program::hash::hash(
        b"global:validate_stat"
    ).to_bytes();
    let mut data = Vec::with_capacity(8);
    data.extend_from_slice(&discriminator[..8]);

    // Serialize the args
    let mut args_data = Vec::new();
    args.serialize(&mut args_data)?;
    data.extend_from_slice(&args_data);

    // Build account metas for CPI
    let account_infos = vec![
        accounts.oracle_proof.to_account_info(),
        accounts.txline_state.to_account_info(),
        accounts.txline_authority.to_account_info(),
    ];

    // Perform the CPI
    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::instruction::Instruction {
            program_id: *txline_program_id,
            accounts: account_infos
                .iter()
                .map(|a| AccountMeta {
                    pubkey: *a.key,
                    is_signer: false,
                    is_writable: false,
                })
                .collect(),
            data,
        },
        &account_infos,
    )?;

    Ok(())
}

/// Generate a leaf hash for a given stat key + value pair.
/// This matches TxLINE's leaf hashing algorithm: keccak256(stat_key || ":" || value)
pub fn compute_txline_leaf(stat_key: &str, value: &str) -> [u8; 32] {
    use anchor_lang::solana_program::keccak;

    let mut hasher = keccak::Hasher::default();
    hasher.hash(stat_key.as_bytes());
    hasher.hash(b":");
    hasher.hash(value.as_bytes());
    hasher.result().to_bytes()
}

/// Verify a Merkle proof against a given root.
///
/// This is a local verification that can be used as a fallback or
/// supplementary check alongside the TxLINE CPI.
///
/// # Arguments
/// * `leaf_hash` - The hash of the leaf node (stat_key + value)
/// * `proof` - Array of sibling hashes forming the Merkle proof path
/// * `root` - The Merkle root to verify against
///
/// # Returns
/// * `bool` - True if the proof is valid
pub fn verify_merkle_proof(leaf_hash: &[u8; 32], proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
    use anchor_lang::solana_program::keccak;

    let mut current_hash = *leaf_hash;

    for sibling in proof {
        let mut hasher = keccak::Hasher::default();
        // Order pairs deterministically (smaller hash first)
        if current_hash < *sibling {
            hasher.hash(&current_hash);
            hasher.hash(sibling);
        } else {
            hasher.hash(sibling);
            hasher.hash(&current_hash);
        }
        current_hash = hasher.result().to_bytes();
    }

    current_hash == *root
}

/// Map a TxLINE resolved value to an outcome index for a market.
///
/// This function interprets the resolved stat value and maps it to
/// one of the market's outcome labels. For binary markets, it checks
/// against "Yes"/"No" or "true"/"false". For multi-outcome markets,
/// it searches the outcome_labels semicolon-delimited list.
///
/// # Arguments
/// * `resolved_value` - The value returned by TxLINE's validate_stat
/// * `outcome_labels` - Semicolon-delimited list of outcome labels
/// * `event_type` - The type of event (Binary, PlayerAction, etc.)
///
/// # Returns
/// * `Option<u8>` - The outcome index, or None if no match found
pub fn map_value_to_outcome(
    resolved_value: &str,
    outcome_labels: &str,
    event_type: u8,
) -> Option<u8> {
    let labels: Vec<&str> = outcome_labels.split(';').collect();

    match event_type {
        0 => {
            // Binary: check for "Yes"/"No" or "true"/"false" or "1"/"0"
            let val_lower = resolved_value.trim().to_lowercase();
            if val_lower == "yes" || val_lower == "true" || val_lower == "1" {
                Some(0)
            } else {
                Some(1)
            }
        }
        1 | 3 => {
            // PlayerAction or MultiChoice: search labels
            let val_trimmed = resolved_value.trim();
            labels
                .iter()
                .position(|label| label.trim().eq_ignore_ascii_case(val_trimmed))
                .map(|i| i as u8)
        }
        2 => {
            // NumericOverUnder: parse numeric and compare to threshold
            if let Ok(value) = resolved_value.trim().parse::<f64>() {
                // First label is "Over X.Y", second is "Under X.Y"
                // Extract threshold from first label
                if let Some(threshold_str) = labels
                    .first()
                    .and_then(|l| l.trim().split_whitespace().nth(1))
                {
                    if let Ok(threshold) = threshold_str.parse::<f64>() {
                        return if value > threshold { Some(0) } else { Some(1) };
                    }
                }
            }
            // Fallback: treat as binary
            Some(1)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_proof_two_leaves() {
        use anchor_lang::solana_program::keccak;
        let leaf_a = compute_txline_leaf("match.goals", "2");
        let leaf_b = compute_txline_leaf("match.corners", "8");

        // Build root from two leaves
        let mut hasher = keccak::Hasher::default();
        if leaf_a < leaf_b {
            hasher.hash(&leaf_a);
            hasher.hash(&leaf_b);
        } else {
            hasher.hash(&leaf_b);
            hasher.hash(&leaf_a);
        }
        let root = hasher.result().to_bytes();

        // Verify leaf_a with proof [leaf_b]
        assert!(verify_merkle_proof(&leaf_a, &[leaf_b], &root));
        // Verify leaf_b with proof [leaf_a]
        assert!(verify_merkle_proof(&leaf_b, &[leaf_a], &root));
        // Wrong root should fail
        let wrong_root = compute_txline_leaf("wrong", "root");
        assert!(!verify_merkle_proof(&leaf_a, &[leaf_b], &wrong_root));
    }

    #[test]
    fn test_map_binary_outcome() {
        assert_eq!(map_value_to_outcome("Yes", "Yes;No", 0), Some(0));
        assert_eq!(map_value_to_outcome("No", "Yes;No", 0), Some(1));
        assert_eq!(map_value_to_outcome("true", "Yes;No", 0), Some(0));
        assert_eq!(map_value_to_outcome("false", "Yes;No", 0), Some(1));
    }

    #[test]
    fn test_map_player_outcome() {
        assert_eq!(
            map_value_to_outcome("Messi", "Messi;Ronaldo;Neymar;No Goal", 1),
            Some(0)
        );
        assert_eq!(
            map_value_to_outcome("Ronaldo", "Messi;Ronaldo;Neymar;No Goal", 1),
            Some(1)
        );
        assert_eq!(
            map_value_to_outcome("Unknown", "Messi;Ronaldo;Neymar;No Goal", 1),
            None
        );
    }

    #[test]
    fn test_map_over_under_outcome() {
        assert_eq!(
            map_value_to_outcome("9", "Over 8.5;Under 8.5", 2),
            Some(0)
        );
        assert_eq!(
            map_value_to_outcome("5", "Over 8.5;Under 8.5", 2),
            Some(1)
        );
    }
}