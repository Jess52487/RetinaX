//! # Post-Quantum Migration Readiness Test Suite
//!
//! ## Cryptographic Assumptions & Quantum Vulnerability Documentation
//!
//! ### 1. Current Pre-Quantum Cryptographic Baseline (BN254 / Groth16 / PLONK)
//! - **Mathematical Foundations**: The current zero-knowledge verification in `zk_verifier`
//!   relies on the BN254 (alt_bn128) pairing-friendly elliptic curve with equation $y^2 = x^3 + 3$.
//! - **Security Hardness Assumptions**:
//!   - Discrete Logarithm Problem (DLP) over groups $G_1$ and $G_2$.
//!   - Computational Diffie-Hellman (CDH) and Bilinear Diffie-Hellman (BDH) over target group $G_T$.
//!   - Knowledge-of-Exponent Assumption (KEA) for Groth16 linear polynomial evaluations.
//! - **Quantum Vulnerability**:
//!   - Shor's algorithm solves abelian group discrete logarithms in polynomial time $\mathcal{O}((\log N)^3)$.
//!   - A cryptanalytically relevant quantum computer (CRQC) with approximately $2,000$ to $4,000$ logical qubits
//!     can recover the toxic waste trapdoor parameters $(\alpha, \beta, \gamma, \delta)$ used in the Groth16 setup ceremony.
//!   - This completely breaks soundness: a quantum attacker can forge valid-looking proofs for arbitrary false statements,
//!     bypassing patient consent and accessing private optometric health records.
//!
//! ### 2. Post-Quantum Migration Strategy
//! - **Target Architecture**: Migration towards Hash-based STARKs (STARK/FRI) and Lattice-based arguments (LWE/SIS).
//! - **Extensible Envelope**: The contract replaces rigid, curve-bound 256-byte `Proof` structures with
//!   an extensible envelope (`ExtensibleProof` / `ProofSystemId`) capable of routing to either classical SNARKs
//!   or quantum-safe STARKs once Soroban instructions allow or native host hash precompiles are deployed.

#![cfg(test)]

use soroban_sdk::{contracttype, testutils::Address as _, Address, BytesN, Env, Vec};
use zk_verifier::{
    vk::{G1Point, G2Point},
    ContractError, ExtensibleProof, Proof, ProofSystemId, ProofValidationError,
};

fn dummy_proof(env: &Env) -> Proof {
    Proof {
        a: G1Point {
            x: BytesN::from_array(env, &[1u8; 32]),
            y: BytesN::from_array(env, &[2u8; 32]),
        },
        b: G2Point {
            x: (
                BytesN::from_array(env, &[1u8; 32]),
                BytesN::from_array(env, &[2u8; 32]),
            ),
            y: (
                BytesN::from_array(env, &[3u8; 32]),
                BytesN::from_array(env, &[4u8; 32]),
            ),
        },
        c: G1Point {
            x: BytesN::from_array(env, &[3u8; 32]),
            y: BytesN::from_array(env, &[4u8; 32]),
        },
    }
}

// ── Scenario 1: Proof struct is extensible for future proof systems ──────────

#[test]
fn test_extensible_proof_envelope_supports_future_proof_systems() {
    let env = Env::default();

    // 1. Classical Groth16 proof
    let groth16_proof = dummy_proof(&env);
    let ext_groth16 = ExtensibleProof::Groth16(groth16_proof);
    assert_eq!(ext_groth16.proof_system_id(), ProofSystemId::Groth16Bn254);
    assert!(
        !ext_groth16.is_post_quantum(),
        "Groth16 must be flagged as non-post-quantum"
    );

    // 2. Future Post-Quantum STARK proof (hash-based FRI commitment root)
    let stark_trace_root = BytesN::from_array(&env, &[0xAA; 32]);
    let ext_stark = ExtensibleProof::Stark(stark_trace_root);
    assert_eq!(ext_stark.proof_system_id(), ProofSystemId::StarkFri);
    assert!(
        ext_stark.is_post_quantum(),
        "STARK with FRI commitment must be recognized as post-quantum secure"
    );

    // 3. Future Post-Quantum Lattice proof (LWE commitment root)
    let lattice_root = BytesN::from_array(&env, &[0xBB; 32]);
    let ext_lattice = ExtensibleProof::Lattice(lattice_root);
    assert_eq!(ext_lattice.proof_system_id(), ProofSystemId::LatticeZk);
    assert!(
        ext_lattice.is_post_quantum(),
        "Lattice-based proof must be recognized as post-quantum secure"
    );
}

#[test]
fn test_proof_system_categorization_and_quantum_flags() {
    // Pre-quantum proof systems (elliptic curve discrete logarithm dependent)
    assert!(!ProofSystemId::Groth16Bn254.is_post_quantum());
    assert!(!ProofSystemId::PlonkBn254.is_post_quantum());

    // Post-quantum proof systems (hash-based and lattice-based)
    assert!(ProofSystemId::StarkFri.is_post_quantum());
    assert!(ProofSystemId::LatticeZk.is_post_quantum());
}

// ── Scenario 2: Invalid proof system identifier handling ─────────────────────

#[test]
fn test_valid_proof_system_identifiers() {
    assert_eq!(
        ProofSystemId::from_u32(1).unwrap(),
        ProofSystemId::Groth16Bn254
    );
    assert_eq!(
        ProofSystemId::from_u32(2).unwrap(),
        ProofSystemId::PlonkBn254
    );
    assert_eq!(ProofSystemId::from_u32(3).unwrap(), ProofSystemId::StarkFri);
    assert_eq!(
        ProofSystemId::from_u32(4).unwrap(),
        ProofSystemId::LatticeZk
    );
}

#[test]
fn test_invalid_proof_system_identifiers_rejected() {
    // 0 is reserved / invalid
    let res_zero = ProofSystemId::from_u32(0);
    assert_eq!(
        res_zero,
        Err(ProofValidationError::InvalidProofSystemId),
        "Identifier 0 must be rejected"
    );

    // 5 is unassigned
    let res_five = ProofSystemId::from_u32(5);
    assert_eq!(
        res_five,
        Err(ProofValidationError::InvalidProofSystemId),
        "Unassigned identifier 5 must be rejected"
    );

    // Arbitrary out of bounds identifiers
    let res_random = ProofSystemId::from_u32(999);
    assert_eq!(
        res_random,
        Err(ProofValidationError::InvalidProofSystemId),
        "Out-of-bounds identifier 999 must be rejected"
    );

    let res_max = ProofSystemId::from_u32(u32::MAX);
    assert_eq!(
        res_max,
        Err(ProofValidationError::InvalidProofSystemId),
        "u32::MAX must be rejected as invalid proof system id"
    );
}

#[test]
fn test_contract_error_mapping_for_invalid_proof_system() {
    // Verify ContractError enum includes InvalidProofSystem with consistent discriminant
    assert_eq!(ContractError::InvalidProofSystem as u32, 15);
}

// ── Scenario 3: Extensible Access Request Simulation ─────────────────────────

/// Demonstrates how the future post-quantum AccessRequest struct
/// replaces hardcoded Groth16 Proof with the extensible proof payload.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PqReadyAccessRequest {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof_system: ProofSystemId,
    pub proof: ExtensibleProof,
    pub public_inputs: Vec<BytesN<32>>,
    pub expires_at: u64,
    pub nonce: u64,
}

#[test]
fn test_pq_ready_access_request_lifecycle() {
    let env = Env::default();
    let user = Address::generate(&env);
    let resource_id = BytesN::from_array(&env, &[0x42; 32]);
    let stark_payload = BytesN::from_array(&env, &[0xEF; 32]);

    let request = PqReadyAccessRequest {
        user: user.clone(),
        resource_id: resource_id.clone(),
        proof_system: ProofSystemId::StarkFri,
        proof: ExtensibleProof::Stark(stark_payload.clone()),
        public_inputs: Vec::new(&env),
        expires_at: 100_000,
        nonce: 1,
    };

    assert_eq!(request.user, user);
    assert_eq!(request.resource_id, resource_id);
    assert_eq!(request.proof_system, ProofSystemId::StarkFri);
    assert!(request.proof_system.is_post_quantum());
    assert_eq!(request.proof, ExtensibleProof::Stark(stark_payload));
}
