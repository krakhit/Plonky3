use alloc::vec;
use alloc::vec::Vec;

use itertools::izip;
use p3_challenger::{CanObserve, CanSample, GrindingChallenger};
use p3_commit::Mmcs;
use p3_field::Field;
use p3_matrix::Dimensions;

use crate::{CommitPhaseProofStep, FriConfig, FriGenericConfig, FriProof};

#[derive(Debug)]
pub enum FriError<CommitMmcsErr> {
    InvalidProofShape,
    CommitPhaseMmcsError(CommitMmcsErr),
    FinalPolyMismatch,
    InvalidPowWitness,
}

pub fn verify<G, F, M, Challenger>(
    g: &G,
    config: &FriConfig<M>,
    proof: &FriProof<F, M, Challenger::Witness, G::InputProof>,
    challenger: &mut Challenger,
    open_input: impl Fn(usize, &G::InputProof) -> Vec<(usize, F)>,
) -> Result<(), FriError<M::Error>>
where
    F: Field,
    M: Mmcs<F>,
    Challenger: GrindingChallenger + CanObserve<M::Commitment> + CanSample<F>,
    G: FriGenericConfig<F>,
{
    let betas: Vec<F> = proof
        .commit_phase_commits
        .iter()
        .map(|comm| {
            challenger.observe(comm.clone());
            challenger.sample()
        })
        .collect();

    if proof.query_proofs.len() != config.num_queries {
        return Err(FriError::InvalidProofShape);
    }

    // Check PoW.
    if !challenger.check_witness(config.proof_of_work_bits, proof.pow_witness) {
        return Err(FriError::InvalidPowWitness);
    }

    let log_max_height = proof.commit_phase_commits.len() + config.log_blowup;

    for qp in &proof.query_proofs {
        let index = challenger.sample_bits(log_max_height + g.extra_query_index_bits());
        let ro = open_input(index, &qp.input_proof);

        let folded_eval = verify_query(
            g,
            config,
            index >> g.extra_query_index_bits(),
            izip!(
                &betas,
                &proof.commit_phase_commits,
                &qp.commit_phase_openings
            ),
            ro,
            log_max_height,
        )?;

        if folded_eval != proof.final_poly {
            return Err(FriError::FinalPolyMismatch);
        }
    }

    Ok(())
}

type CommitStep<'a, F, M> = (
    &'a F,
    &'a <M as Mmcs<F>>::Commitment,
    &'a CommitPhaseProofStep<F, M>,
);

fn verify_query<'a, G, F, M>(
    g: &G,
    config: &FriConfig<M>,
    mut index: usize,
    steps: impl Iterator<Item = CommitStep<'a, F, M>>,
    reduced_openings: Vec<(usize, F)>,
    log_max_height: usize,
) -> Result<F, FriError<M::Error>>
where
    F: Field,
    M: Mmcs<F> + 'a,
    G: FriGenericConfig<F>,
{
    let mut folded_eval = F::zero();
    let mut ro_iter = reduced_openings.into_iter().peekable();

    for (log_folded_height, (&beta, comm, opening)) in izip!((0..log_max_height).rev(), steps) {
        if let Some((_, ro)) = ro_iter.next_if(|(lh, _)| *lh == log_folded_height + 1) {
            folded_eval += ro;
        }

        let index_sibling = index ^ 1;
        let index_pair = index >> 1;

        let mut evals = vec![folded_eval; 2];
        evals[index_sibling % 2] = opening.sibling_value;

        let dims = &[Dimensions {
            width: 2,
            height: 1 << log_folded_height,
        }];
        config
            .mmcs
            .verify_batch(
                comm,
                dims,
                index_pair,
                &[evals.clone()],
                &opening.opening_proof,
            )
            .map_err(FriError::CommitPhaseMmcsError)?;

        index = index_pair;

        folded_eval = g.fold_row(index, log_folded_height, beta, evals.into_iter());
    }

    debug_assert!(index < config.blowup(), "index was {}", index);
    debug_assert!(
        ro_iter.next().is_none(),
        "verifier reduced_openings were not in descending order?"
    );

    Ok(folded_eval)
}
