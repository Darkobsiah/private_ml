use fawkes_crypto::{
    backend::bellman_groth16::{
        prover,
        engines::{Bn256},
        setup::setup
    },
    circuit::bool::CBool,
    circuit::cs::{CS, RCS},
    circuit::num::CNum,
    circuit::bitify::c_into_bits_le_strict,
    circuit::ecc::*,
    core::signal::Signal,
    core::sizedvec::SizedVec,
    engines::bn256::{JubJubBN256},
    native::ecc::*,
    rand::{thread_rng, Rng},
    ff_uint::{Num, PrimeFieldParams, PrimeField},
};

use std::str::FromStr;
use num_bigint::{BigInt}; 
use crate::math::matrix::{BigIntMatrix};
use crate::utils::{quadratic_result, reduce};

use super::SnarkInfo;

type Fr = fawkes_crypto::engines::bn256::Fr;
type E = Bn256;
type JJParams = JubJubBN256;


#[derive(Clone, Debug)]
pub struct QPProofPublic<Fr: PrimeField, const L: usize> {
    pub g1: EdwardsPoint<Fr>,
    pub h1: EdwardsPoint<Fr>,
    pub c1: EdwardsPoint<Fr>,
    pub c2: EdwardsPoint<Fr>,
    pub c3: SizedVec<EdwardsPoint<Fr>, L>,
    pub c4: SizedVec<EdwardsPoint<Fr>, L>
}

#[derive(Clone, Signal)]
#[Value = "QPProofPublic<C::Fr, L>"]
pub struct CQPProofPublic<C: CS, const L: usize> {
    pub g1: CEdwardsPoint<C>,
    pub h1: CEdwardsPoint<C>,
    pub c1: CEdwardsPoint<C>,
    pub c2: CEdwardsPoint<C>,
    pub c3: SizedVec<CEdwardsPoint<C>, L>,
    pub c4: SizedVec<CEdwardsPoint<C>, L>
}

#[derive(Clone, Debug)]
pub struct QPProofSecret<Fr: PrimeField, const L: usize> {
    pub r: Num<Fr>,
    pub f_st: Num<Fr>,
    pub s: SizedVec<Num<Fr>, L>,
    pub t: SizedVec<Num<Fr>, L>
}

#[derive(Clone, Signal)]
#[Value = "QPProofSecret<C::Fr, L>"]
pub struct CQPProofSecret<C: CS, const L: usize> {
    pub r: CNum<C>,
    pub f_st: CNum<C>,
    pub s: SizedVec<CNum<C>, L>,
    pub t: SizedVec<CNum<C>, L>
}

pub struct ZkQp<const L: usize>;

impl<const L: usize> ZkQp<L> {

    fn circuit<C: CS<Fr = Fr>>(public: CQPProofPublic<C, L>, secret: CQPProofSecret<C, L>) {
        let jubjub_params = JJParams::new();

        let f_st_bits = c_into_bits_le_strict(&secret.f_st);
        let r_bits = c_into_bits_le_strict(&secret.r);

        let c1 = public.g1.mul(&f_st_bits, &jubjub_params)
            .add(&public.h1.mul(&r_bits, &jubjub_params), &jubjub_params);
        c1.assert_eq(&public.c1);

        let c2 = public.g1.mul(&r_bits, &jubjub_params); 
        c2.assert_eq(&public.c2);

        let c3 = secret.s.iter()
            .map(|si| public.g1.mul(&c_into_bits_le_strict(si), &jubjub_params))
            .collect::<SizedVec<CEdwardsPoint<C>, L>>();
        c3.assert_eq(&public.c3);

        let c4 = secret.t.iter()
            .map(|ti| public.g1.mul(&c_into_bits_le_strict(ti), &jubjub_params))
            .collect::<SizedVec<CEdwardsPoint<C>, L>>();
        c4.assert_eq(&public.c4);
    }

    pub fn generate(g1: &EdwardsPoint<Fr>, h1: &EdwardsPoint<Fr>, s: &SizedVec<Num<Fr>, L>, t: &SizedVec<Num<Fr>, L>, f: &BigIntMatrix) -> SnarkInfo<E> {
        let jubjub_params = JJParams::new();
        let mut rng = thread_rng();

        let r: Num<Fr> = rng.gen();

        let bigint_mod = BigInt::from_str(&Fr::MODULUS.to_string()).unwrap();
        let bigint_s: Vec<BigInt> = s.iter().map(|x| BigInt::from_str(&x.to_string()).unwrap()).collect(); 
        let bigint_t: Vec<BigInt> = t.iter().map(|x| BigInt::from_str(&x.to_string()).unwrap()).collect();
        let bigint_result = reduce(&quadratic_result(&bigint_s, &bigint_t, &f), &bigint_mod);
        let f_st = Num::<Fr>::from_str(&bigint_result.to_string()).unwrap();
        println!("bigint_f(s, t): {}", bigint_result);
        println!("f(s, t): {}", f_st);

        let c1 = g1.mul(f_st.to_other_reduced(), &jubjub_params)
            .add(&h1.mul(r.to_other_reduced(), &jubjub_params), &jubjub_params);
        let c2 = g1.mul(r.to_other_reduced(), &jubjub_params);
        let c3 = s.iter()
            .map(|si| g1.mul(si.to_other_reduced(), &jubjub_params))
            .collect::<SizedVec<_, L>>();
        let c4 = t.iter()
            .map(|ti| g1.mul(ti.to_other_reduced(), &jubjub_params))
            .collect::<SizedVec<_, L>>();

        let qp_proof_public = QPProofPublic {
            g1: g1.clone(),
            h1: h1.clone(),
            c1,
            c2,
            c3,
            c4
        };
        let qp_proof_secret = QPProofSecret {
            r,
            f_st,
            s: s.clone(),
            t: t.clone()
        };

        let bellman_params = setup::<E, _, _, _>(ZkQp::<L>::circuit);
        let (inputs, snark_proof) = prover::prove(&bellman_params, &qp_proof_public, &qp_proof_secret, ZkQp::<L>::circuit);
        SnarkInfo::<E> {
            inputs: inputs,
            proof: snark_proof,
            vk: bellman_params.get_vk()
        }
    }
}