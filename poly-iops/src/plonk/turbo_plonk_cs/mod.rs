/// This file implements a Turbo PLONK constraint system. It also implements a set of
/// arithmetic/boolean/range gates that will be used in Anonymous transfer.
/// The gates for elliptic curve operations and Rescue cipher/hash functions are implemented
/// in ecc.rs and rescue.rs, respectively.
pub mod ecc;

pub mod rescue;

use crate::plonk::errors::PlonkError;
use crate::plonk::plonk_setup::ConstraintSystem;
use algebra::groups::Scalar;
use ruc::*;

pub type VarIndex = usize; // Variable index
pub type CsIndex = usize; // Constraint index

pub const N_WIRES_PER_GATE: usize = 5;
pub const N_SELECTORS: usize = 13;

#[derive(Serialize, Deserialize)]
pub struct TurboPlonkConstraintSystem<F> {
    pub selectors: Vec<Vec<F>>,
    pub wiring: [Vec<VarIndex>; N_WIRES_PER_GATE],
    pub num_vars: usize,
    pub size: usize,
    pub public_vars_constraint_indices: Vec<CsIndex>,
    pub public_vars_witness_indices: Vec<VarIndex>,
    // A private witness for the circuit, cleared after computing a proof
    witness: Vec<F>,
    // A reserved variable that maps to value zero
    zero_var: Option<VarIndex>,
    // A reserved variable that maps to value one
    one_var: Option<VarIndex>,
}

impl<F: Scalar> ConstraintSystem for TurboPlonkConstraintSystem<F> {
    type Field = F;

    fn size(&self) -> usize {
        self.size
    }

    fn num_vars(&self) -> usize {
        self.num_vars
    }

    fn wiring(&self) -> &[Vec<usize>] {
        &self.wiring[..]
    }

    /// `quot_eval_dom_size` divides (q-1), and should be larger than the degree of the quotient
    /// polynomial, i.e., `quot_eval_dom_size` > 5 * `self.size` + 7
    fn quot_eval_dom_size(&self) -> usize {
        if self.size > 4 {
            self.size * 6
        } else {
            self.size * 16
        }
    }

    fn n_wires_per_gate(&self) -> usize {
        N_WIRES_PER_GATE
    }

    fn num_selectors(&self) -> usize {
        N_SELECTORS
    }

    fn public_vars_constraint_indices(&self) -> &[CsIndex] {
        &self.public_vars_constraint_indices
    }

    fn public_vars_witness_indices(&self) -> &[VarIndex] {
        &self.public_vars_witness_indices
    }

    fn selector(&self, index: usize) -> Result<&[F]> {
        if index >= self.selectors.len() {
            return Err(eg!(PlonkError::FuncParamsError));
        }
        Ok(&self.selectors[index])
    }

    /// The equation is
    /// ```text
    ///     q1*w1 + q2*w2 + q3*w3 + q4*w4 + qm1(w1*w2) + qm2(w3*w4) + qc + PI
    ///     + q_ecc*[w1*w2*w3*w4*wo]
    ///     + q_hash_1 * w1^5 + q_hash_2 * w2^5 + q_hash_3 * w3^5 + q_hash_4 * w4^5
    ///     - qo * wo = 0
    /// ```
    fn eval_gate_func(
        &self,
        wire_vals: &[&F],
        sel_vals: &[&F],
        pub_input: &F,
    ) -> Result<F> {
        if wire_vals.len() != N_WIRES_PER_GATE || sel_vals.len() != N_SELECTORS {
            return Err(eg!(PlonkError::FuncParamsError));
        }
        let add1 = sel_vals[0].mul(&wire_vals[0]);
        let add2 = sel_vals[1].mul(&wire_vals[1]);
        let add3 = sel_vals[2].mul(&wire_vals[2]);
        let add4 = sel_vals[3].mul(&wire_vals[3]);
        let mul1 = sel_vals[4].mul(&wire_vals[0].mul(&wire_vals[1]));
        let mul2 = sel_vals[5].mul(&wire_vals[2].mul(&wire_vals[3]));
        let constant = sel_vals[6].add(pub_input);
        let ecc = sel_vals[7]
            .mul(&wire_vals[0])
            .mul(&wire_vals[1])
            .mul(&wire_vals[2])
            .mul(&wire_vals[3])
            .mul(&wire_vals[4]);
        let five = &[5u64];
        let hash1 = sel_vals[8].mul(&wire_vals[0].pow(five));
        let hash2 = sel_vals[9].mul(&wire_vals[1].pow(five));
        let hash3 = sel_vals[10].mul(&wire_vals[2].pow(five));
        let hash4 = sel_vals[11].mul(&wire_vals[3].pow(five));
        let out = sel_vals[12].mul(&wire_vals[4]);
        let mut r = add1;
        r.add_assign(&add2);
        r.add_assign(&add3);
        r.add_assign(&add4);
        r.add_assign(&mul1);
        r.add_assign(&mul2);
        r.add_assign(&ecc);
        r.add_assign(&hash1);
        r.add_assign(&hash2);
        r.add_assign(&hash3);
        r.add_assign(&hash4);
        r.add_assign(&constant);
        r.sub_assign(&out);
        Ok(r)
    }

    /// The coefficients are
    /// (w1, w2, w3, w4, w1*w2, w3*w4, 1, w1*w2*w3*w4*wo, w1^5, w2^5, w3^5, w4^5, -w4)
    fn eval_selector_multipliers(&self, wire_vals: &[&F]) -> Result<Vec<F>> {
        if wire_vals.len() < N_WIRES_PER_GATE {
            return Err(eg!(PlonkError::FuncParamsError));
        }
        let five = &[5u64];
        let mut w0w1w2w3w4 = *wire_vals[0];
        w0w1w2w3w4.mul_assign(wire_vals[1]);
        w0w1w2w3w4.mul_assign(wire_vals[2]);
        w0w1w2w3w4.mul_assign(wire_vals[3]);
        w0w1w2w3w4.mul_assign(wire_vals[4]);
        Ok(vec![
            *wire_vals[0],
            *wire_vals[1],
            *wire_vals[2],
            *wire_vals[3],
            wire_vals[0].mul(wire_vals[1]),
            wire_vals[2].mul(wire_vals[3]),
            F::one(),
            w0w1w2w3w4,
            wire_vals[0].pow(five),
            wire_vals[1].pow(five),
            wire_vals[2].pow(five),
            wire_vals[3].pow(five),
            wire_vals[4].neg(),
        ])
    }
}

// A helper function that computes the little-endian binary representation of a value.
// Each bit is represented as a field element.
fn compute_binary_le<F: Scalar>(bytes: &[u8]) -> Vec<F> {
    let mut res = vec![];
    for byte in bytes.iter() {
        let mut tmp = *byte;
        for _ in 0..8 {
            if (tmp & 1) == 0 {
                res.push(F::zero());
            } else {
                res.push(F::one());
            }
            tmp >>= 1;
        }
    }
    res
}

impl<F: Scalar> Default for TurboPlonkConstraintSystem<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Scalar> TurboPlonkConstraintSystem<F> {
    /// Create a TurboPLONK constraint system with a certain field size.
    pub fn new() -> TurboPlonkConstraintSystem<F> {
        let selectors: Vec<Vec<F>> =
            std::iter::repeat(vec![]).take(N_SELECTORS).collect();
        TurboPlonkConstraintSystem {
            selectors,
            wiring: [vec![], vec![], vec![], vec![], vec![]],
            num_vars: 0,
            size: 0,
            public_vars_constraint_indices: vec![],
            public_vars_witness_indices: vec![],
            witness: vec![],
            zero_var: None,
            one_var: None,
        }
    }

    pub fn zero_var(&mut self) -> VarIndex {
        if self.zero_var.is_none() {
            self.zero_var = Some(self.num_vars);
            self.witness.push(F::zero());
            self.num_vars += 1;
        }
        self.zero_var.unwrap() // safe unwrap
    }

    pub fn one_var(&mut self) -> VarIndex {
        if self.one_var.is_none() {
            self.one_var = Some(self.num_vars);
            self.witness.push(F::one());
            self.num_vars += 1;
        }
        self.one_var.unwrap() // safe unwrap
    }

    /// Insert a linear combination gate: wo = w1 * q1 + w2 * q2 + w3 * q3 + w4 * q4.
    pub fn insert_lc_gate(
        &mut self,
        wires_in: &[VarIndex; 4],
        wire_out: VarIndex,
        q1: F,
        q2: F,
        q3: F,
        q4: F,
    ) {
        assert!(
            wires_in.iter().all(|&x| x < self.num_vars),
            "input wire index out of bound"
        );
        assert!(wire_out < self.num_vars, "wire_out index out of bound");
        let zero = F::zero();
        self.push_add_selectors(q1, q2, q3, q4);
        self.push_mul_selectors(zero, zero);
        self.push_constant_selector(zero);
        self.push_ecc_selector(zero);
        self.push_rescue_selectors(zero, zero, zero, zero);
        self.push_out_selector(F::one());
        for (i, wire) in wires_in.iter().enumerate() {
            self.wiring[i].push(*wire);
        }
        self.wiring[4].push(wire_out);
        self.size += 1;
    }

    pub fn insert_add_gate(
        &mut self,
        left_var: VarIndex,
        right_var: VarIndex,
        out_var: VarIndex,
    ) {
        self.insert_lc_gate(
            &[left_var, right_var, 0, 0],
            out_var,
            F::one(),
            F::one(),
            F::zero(),
            F::zero(),
        );
    }

    pub fn insert_sub_gate(
        &mut self,
        left_var: VarIndex,
        right_var: VarIndex,
        out_var: VarIndex,
    ) {
        self.insert_lc_gate(
            &[left_var, right_var, 0, 0],
            out_var,
            F::one(),
            F::one().neg(),
            F::zero(),
            F::zero(),
        );
    }

    pub fn insert_mul_gate(
        &mut self,
        left_var: VarIndex,
        right_var: VarIndex,
        out_var: VarIndex,
    ) {
        assert!(left_var < self.num_vars, "left_var index out of bound");
        assert!(right_var < self.num_vars, "right_var index out of bound");
        assert!(out_var < self.num_vars, "out_var index out of bound");
        let zero = F::zero();
        self.push_add_selectors(zero, zero, zero, zero);
        self.push_mul_selectors(F::one(), zero);
        self.push_constant_selector(zero);
        self.push_ecc_selector(zero);
        self.push_rescue_selectors(zero, zero, zero, zero);
        self.push_out_selector(F::one());
        self.wiring[0].push(left_var);
        self.wiring[1].push(right_var);
        self.wiring[2].push(0);
        self.wiring[3].push(0);
        self.wiring[4].push(out_var);
        self.size += 1;
    }

    /// Add a variable (with actual value `value`) into the constraint system.
    pub fn new_variable(&mut self, value: F) -> VarIndex {
        self.num_vars += 1;
        self.witness.push(value);
        self.num_vars - 1
    }

    /// Add a vector of variables into the constraint system.
    pub fn add_variables(&mut self, values: &[F]) {
        self.num_vars += values.len();
        for value in values.iter() {
            self.witness.push(*value);
        }
    }

    /// Create an output variable and insert a linear combination gate.
    pub fn linear_combine(
        &mut self,
        wires_in: &[VarIndex; 4],
        q1: F,
        q2: F,
        q3: F,
        q4: F,
    ) -> VarIndex {
        assert!(
            wires_in.iter().all(|&x| x < self.num_vars),
            "input wire index out of bound"
        );
        let w0q1 = self.witness[wires_in[0]].mul(&q1);
        let w1q2 = self.witness[wires_in[1]].mul(&q2);
        let w2q3 = self.witness[wires_in[2]].mul(&q3);
        let w3q4 = self.witness[wires_in[3]].mul(&q4);
        let mut lc = w0q1;
        lc.add_assign(&w1q2);
        lc.add_assign(&w2q3);
        lc.add_assign(&w3q4);
        let wire_out = self.new_variable(lc);
        self.insert_lc_gate(wires_in, wire_out, q1, q2, q3, q4);
        wire_out
    }

    /// Create an output variable and insert an addition gate.
    pub fn add(&mut self, left_var: VarIndex, right_var: VarIndex) -> VarIndex {
        assert!(left_var < self.num_vars, "left_var index out of bound");
        assert!(right_var < self.num_vars, "right_var index out of bound");
        let out_var =
            self.new_variable(self.witness[left_var].add(&self.witness[right_var]));
        self.insert_add_gate(left_var, right_var, out_var);
        out_var
    }

    /// Create an output variable and insert a subraction gate.
    pub fn sub(&mut self, left_var: VarIndex, right_var: VarIndex) -> VarIndex {
        assert!(left_var < self.num_vars, "left_var index out of bound");
        assert!(right_var < self.num_vars, "right_var index out of bound");
        let out_var =
            self.new_variable(self.witness[left_var].sub(&self.witness[right_var]));
        self.insert_sub_gate(left_var, right_var, out_var);
        out_var
    }

    // Add a constraint that `left_var` and `right_var` have the same value.
    pub fn equal(&mut self, left_var: VarIndex, right_var: VarIndex) {
        let zero_var = self.zero_var();
        self.insert_sub_gate(left_var, right_var, zero_var);
    }

    /// Create an output variable and insert a multiplication gate.
    pub fn mul(&mut self, left_var: VarIndex, right_var: VarIndex) -> VarIndex {
        assert!(left_var < self.num_vars, "left_var index out of bound");
        assert!(right_var < self.num_vars, "right_var index out of bound");
        let out_var =
            self.new_variable(self.witness[left_var].mul(&self.witness[right_var]));
        self.insert_mul_gate(left_var, right_var, out_var);
        out_var
    }

    /// Boolean constrain `var` by adding a multiplication gate:
    /// `witness[var] * witness[var] = witness[var]`
    pub fn insert_boolean_gate(&mut self, var: VarIndex) {
        self.insert_mul_gate(var, var, var);
    }

    /// Enforce a range constraint: `0 < witness[var] < 2^n_bits`:
    /// 1. Transform `witness[var]` into a binary vector and boolean constrain the binary vector.
    /// 2. Adding a set of linear combination constraints showing that the binary vector is a binary
    /// representation of `witness[var]`.
    /// 3. Return witness indices of the binary vector. The binary vector is in little endian form.
    pub fn range_check(&mut self, var: VarIndex, n_bits: usize) -> Vec<VarIndex> {
        assert!(var < self.num_vars, "var index out of bound");
        assert!(n_bits >= 2, "the number of bits is less than two");
        let witness_bytes = self.witness[var].to_bytes();
        let mut binary_repr = compute_binary_le::<F>(&witness_bytes);
        while binary_repr.len() < n_bits {
            binary_repr.push(F::zero());
        }

        let b: Vec<VarIndex> = binary_repr
            .into_iter()
            .take(n_bits)
            .map(|val| self.new_variable(val))
            .collect();

        for elem in b.iter() {
            self.insert_boolean_gate(*elem);
        }
        let one = F::one();
        let two = one.add(&one);
        let four = two.add(&two);
        let eight = four.add(&four);
        let bin = vec![one, two, four, eight];

        let mut acc = b[n_bits - 1];
        let m = (n_bits - 2) / 3;
        for i in 0..m {
            acc = self.linear_combine(
                &[
                    acc,
                    b[n_bits - 1 - i * 3 - 1],
                    b[n_bits - 1 - i * 3 - 2],
                    b[n_bits - 1 - i * 3 - 3],
                ],
                bin[3],
                bin[2],
                bin[1],
                bin[0],
            );
        }
        let zero = F::zero();
        match (n_bits - 1) - 3 * m {
            1 => {
                self.insert_lc_gate(&[acc, b[0], 0, 0], var, bin[1], bin[0], zero, zero)
            }
            2 => self.insert_lc_gate(
                &[acc, b[1], b[0], 0],
                var,
                bin[2],
                bin[1],
                bin[0],
                zero,
            ),
            _ => self.insert_lc_gate(
                &[acc, b[2], b[1], b[0]],
                var,
                bin[3],
                bin[2],
                bin[1],
                bin[0],
            ),
        }
        b
    }

    /// Given two variables `var0` and `var1` and a boolean variable `bit`, return var_bit.
    /// var_bit = (1-bit) * var0 + bit * var1 = - bit * var0 + bit * var1 + var0
    /// Wires: (w1, w2, w3 , w4) = (bit, var0, bit, var1)
    /// Selectors: q2 = qm2 = qo = 1, qm1 = -1
    pub fn select(&mut self, var0: VarIndex, var1: VarIndex, bit: VarIndex) -> VarIndex {
        assert!(var0 < self.num_vars, "var0 index out of bound");
        assert!(var1 < self.num_vars, "var1 index out of bound");
        assert!(bit < self.num_vars, "bit var index out of bound");
        let zero = F::zero();
        let one = F::one();
        self.push_add_selectors(zero, one, zero, zero);
        self.push_mul_selectors(one.neg(), one);
        self.push_constant_selector(zero);
        self.push_ecc_selector(zero);
        self.push_rescue_selectors(zero, zero, zero, zero);
        self.push_out_selector(one);
        let out = if self.witness[bit] == zero {
            self.witness[var0]
        } else {
            self.witness[var1]
        };
        let out_var = self.new_variable(out);
        self.wiring[0].push(bit);
        self.wiring[1].push(var0);
        self.wiring[2].push(bit);
        self.wiring[3].push(var1);
        self.wiring[4].push(out_var);
        self.size += 1;
        out_var
    }

    // Returns a boolean variable that equals 1 if and only if `left_var` == `right_var`
    pub fn is_equal(&mut self, left_var: VarIndex, right_var: VarIndex) -> VarIndex {
        let (is_equal, _) = self.is_equal_or_not_equal(left_var, right_var);
        is_equal
    }

    // Returns a boolean variable that equals 1 if and only if `left_var` != `right_var`
    pub fn is_not_equal(&mut self, left_var: VarIndex, right_var: VarIndex) -> VarIndex {
        let (_, is_not_equal) = self.is_equal_or_not_equal(left_var, right_var);
        is_not_equal
    }

    // Returns two boolean variables that equals (1, 0) if and only if `left_var` == `right_var` and (0, 1) otherwise
    pub fn is_equal_or_not_equal(
        &mut self,
        left_var: VarIndex,
        right_var: VarIndex,
    ) -> (VarIndex, VarIndex) {
        let diff = self.sub(left_var, right_var);
        // set `inv_diff` = `diff`^{-1} when `diff` != 0, otherwise we can set `inv_diff` to arbirary value since `diff` * `inv_diff` will always be 0 when `diff` == 0
        let inv_diff_scalar = self.witness[diff].inv().unwrap_or_else(|_| F::zero());
        let inv_diff = self.new_variable(inv_diff_scalar);

        // `diff_is_zero` = 1 - `diff` * `inv_diff`
        // `diff_is_zero` will be 1 when `diff` == 0, and `diff_is_zero` will be 0 when `diff != 0` and `inv_diff` == `diff`^{-1}
        let mul_var = self.mul(diff, inv_diff);
        let one_var = self.one_var();
        let diff_is_zero = self.sub(one_var, mul_var);

        // enforce `diff` * `diff_is_zero` == 0
        // without this constraint, a malicious prover can set `diff_is_zero` to arbitrary value when `diff` != 0
        let zero_var = self.zero_var();
        self.insert_mul_gate(diff, diff_is_zero, zero_var);

        (diff_is_zero, mul_var)
    }

    /// Insert a constant constraint: wo = constant
    pub fn insert_constant_gate(&mut self, var: VarIndex, constant: F) {
        assert!(var < self.num_vars, "variable index out of bound");
        let zero = F::zero();
        self.push_add_selectors(zero, zero, zero, zero);
        self.push_mul_selectors(zero, zero);
        self.push_constant_selector(constant);
        self.push_ecc_selector(zero);
        self.push_rescue_selectors(zero, zero, zero, zero);
        self.push_out_selector(F::one());
        for i in 0..N_WIRES_PER_GATE {
            self.wiring[i].push(var);
        }
        self.size += 1;
    }

    /// Insert constraint of a public IO value to be decided online.
    pub fn prepare_io_variable(&mut self, var: VarIndex) {
        self.public_vars_witness_indices.push(var);
        self.public_vars_constraint_indices.push(self.size);
        self.insert_constant_gate(var, F::zero());
    }

    /// Pad the number of constraints to a power of two.
    pub fn pad(&mut self) {
        let n = self.size.next_power_of_two();
        let diff = n - self.size();
        for selector in self.selectors.iter_mut() {
            selector.extend(vec![F::zero(); diff]);
        }
        for wire in self.wiring.iter_mut() {
            wire.extend(vec![0; diff]);
        }
        self.size += diff;
    }

    fn push_add_selectors(&mut self, q1: F, q2: F, q3: F, q4: F) {
        self.selectors[0].push(q1);
        self.selectors[1].push(q2);
        self.selectors[2].push(q3);
        self.selectors[3].push(q4);
    }

    fn push_mul_selectors(&mut self, q_mul12: F, q_mul34: F) {
        self.selectors[4].push(q_mul12);
        self.selectors[5].push(q_mul34);
    }

    fn push_constant_selector(&mut self, q_c: F) {
        self.selectors[6].push(q_c);
    }

    fn push_ecc_selector(&mut self, q_ecc: F) {
        self.selectors[7].push(q_ecc);
    }

    fn push_rescue_selectors(
        &mut self,
        q_hash_1: F,
        q_hash_2: F,
        q_hash_3: F,
        q_hash_4: F,
    ) {
        self.selectors[8].push(q_hash_1);
        self.selectors[9].push(q_hash_2);
        self.selectors[10].push(q_hash_3);
        self.selectors[11].push(q_hash_4);
    }

    fn push_out_selector(&mut self, q_out: F) {
        self.selectors[12].push(q_out);
    }

    fn get_witness_index(&self, wire_index: usize, cs_index: CsIndex) -> VarIndex {
        assert!(wire_index < N_WIRES_PER_GATE, "wire index out of bound");
        assert!(cs_index < self.size, "constraint index out of bound");
        self.wiring[wire_index][cs_index]
    }

    pub fn verify_witness(&self, witness: &[F], online_vars: &[F]) -> Result<()> {
        if witness.len() != self.num_vars {
            return Err(eg!(format!(
                "witness len = {}, num_vars = {}",
                witness.len(),
                self.num_vars
            )));
        }
        if online_vars.len() != self.public_vars_witness_indices.len()
            || online_vars.len() != self.public_vars_constraint_indices.len()
        {
            return Err(eg!("wrong number of online variables"));
        }
        for cs_index in 0..self.size() {
            let mut public_online = F::zero();
            // check if the constraint constrains a public variable
            // search constraint index in online vars
            for ((c_i, w_i), online_var) in self
                .public_vars_constraint_indices
                .iter()
                .zip(self.public_vars_witness_indices.iter())
                .zip(online_vars.iter())
            {
                if *c_i == cs_index {
                    // found
                    public_online = *online_var;
                    if witness[*w_i] != *online_var {
                        return Err(eg!(format!(
                            "cs index {}: online var {:?} does not match witness {:?}",
                            cs_index, *online_var, witness[*w_i]
                        )));
                    }
                }
            }
            let w1_value = &witness[self.get_witness_index(0, cs_index)];
            let w2_value = &witness[self.get_witness_index(1, cs_index)];
            let w3_value = &witness[self.get_witness_index(2, cs_index)];
            let w4_value = &witness[self.get_witness_index(3, cs_index)];
            let w_out_value = &witness[self.get_witness_index(4, cs_index)];
            let wire_vals = vec![w1_value, w2_value, w3_value, w4_value, w_out_value];
            let sel_vals: Vec<&F> = (0..self.num_selectors())
                .map(|i| &self.selectors[i][cs_index])
                .collect();
            let eval_gate = self
                .eval_gate_func(&wire_vals, &sel_vals, &public_online)
                .c(d!("wrong func params for eval_gate_func()"))?;
            if eval_gate != F::zero() {
                return Err(eg!(format!(
                    "cs index {}: wire_vals = ({:?}), sel_vals = ({:?})",
                    cs_index, wire_vals, sel_vals
                )));
            }
        }
        Ok(())
    }

    /// Extract and clear the entire witness of the circuit. The witness consists of
    /// secret inputs, public inputs, and the values of intermediate variables.
    pub fn get_and_clear_witness(&mut self) -> Vec<F> {
        let res = self.witness.clone();
        self.witness.clear();
        res
    }
}

#[cfg(test)]
mod test {
    use crate::plonk::turbo_plonk_cs::TurboPlonkConstraintSystem;
    use algebra::bls12_381::BLSScalar;
    use algebra::groups::{Scalar, ScalarArithmetic};
    use ruc::*;

    type F = BLSScalar;
    #[test]
    fn test_select() {
        let mut cs = TurboPlonkConstraintSystem::new();
        let num: Vec<F> = (0..4).map(|x| F::from_u32(x as u32)).collect();
        cs.new_variable(num[0]); // bit0 = 0
        cs.new_variable(num[1]); // bit1 = 1
        cs.new_variable(num[2]); // var0
        cs.new_variable(num[3]); // var1

        // select(var0, var1, bit0)
        let a_idx = cs.select(2, 3, 0);
        assert_eq!(cs.witness[a_idx], num[2]);
        // select(var0, var1, bit1)
        let b_idx = cs.select(2, 3, 1);
        assert_eq!(cs.witness[b_idx], num[3]);

        assert!(cs
            .verify_witness(&[num[0], num[1], num[2], num[3], num[2], num[3]], &[])
            .is_ok());

        // Set bit0 = 1 and bit1 = 0
        assert!(cs
            .verify_witness(&[num[1], num[0], num[2], num[3], num[3], num[2]], &[])
            .is_ok());

        assert!(cs
            .verify_witness(&[num[0], num[1], num[2], num[3], num[3], num[2]], &[])
            .is_err());
    }

    #[test]
    fn test_sub_and_equal() {
        let mut cs = TurboPlonkConstraintSystem::new();
        let zero = F::from_u32(0);
        let one = F::from_u32(1);
        let two = one.add(&one);
        let three = two.add(&one);
        cs.new_variable(zero);
        cs.new_variable(one);
        cs.new_variable(two);
        cs.new_variable(three);
        let add = cs.add(0, 2);
        let sub = cs.sub(3, 1);
        cs.equal(add, sub);

        let witness = cs.get_and_clear_witness();
        pnk!(cs.verify_witness(&witness[..], &[]));

        assert!(cs
            .verify_witness(&[zero, one, two, two, two, one, zero], &[])
            .is_err());
    }

    #[test]
    fn test_is_equal() {
        let mut cs = TurboPlonkConstraintSystem::new();
        let zero = F::from_u32(0);
        let one = F::from_u32(1);
        let two = one.add(&one);
        cs.new_variable(one);
        cs.new_variable(two);
        cs.new_variable(two);
        let one_equals_two = cs.is_equal(0, 1);
        assert_eq!(cs.witness[one_equals_two], zero);
        let two_equals_two = cs.is_equal(1, 2);
        assert_eq!(cs.witness[two_equals_two], one);

        let mut witness = cs.get_and_clear_witness();
        pnk!(cs.verify_witness(&witness, &[]));

        witness[0] = two;
        assert!(cs.verify_witness(&witness, &[]).is_err());
    }

    #[test]
    fn test_turbo_plonk_circuit_1() {
        let mut cs = TurboPlonkConstraintSystem::new();
        let num: Vec<F> = (0..6).map(|x| F::from_u32(x as u32)).collect();

        // The circuit description:
        // 1. c = add(a, b)
        // 2. d = mul(a, b)
        // 3. e = linear_combine(a, b, c, d)
        // 4. 0 <= e < 8
        // The secret inputs: [a, b] = [1, 1]
        cs.new_variable(num[1]);
        cs.new_variable(num[1]);
        let c_idx = cs.add(0, 1);
        let d_idx = cs.mul(0, 1);
        let e_idx =
            cs.linear_combine(&[0, 1, c_idx, d_idx], num[1], num[1], num[1], num[1]);

        cs.range_check(e_idx, 3);

        let witness = cs.get_and_clear_witness();
        pnk!(cs.verify_witness(&witness[..], &[]));

        let eight = num[3].add(&num[5]);
        // Bad witness: [a, b] = [1, 2], [c, d, e] = [3, 2, 8] and e >= 8
        // set e_binary = [1, 1, 1]
        assert!(cs
            .verify_witness(
                &[num[1], num[2], num[3], num[2], eight, num[1], num[1], num[1]],
                &[]
            )
            .is_err());

        /*
        // Bad witness: a + b != c
        assert!(cs.verify_witness(&[num[1].clone(),
                                    num[1].clone(),
                                    num[1].clone(),
                                    num[1].clone(),
                                    num[4].clone(),
                                    num[0].clone(),
                                    num[0].clone(),
                                    num[1].clone()],
                                  &[])
                  .is_err());

        // Bad witness: a * b != d
        assert!(cs.verify_witness(&[num[1].clone(),
                                    num[1].clone(),
                                    num[2].clone(),
                                    num[0].clone(),
                                    num[4].clone(),
                                    num[0].clone(),
                                    num[0].clone(),
                                    num[1].clone()],
                                  &[])
                  .is_err());
        // Bad witness: a + b + c + d != e
        assert!(cs.verify_witness(&[num[1].clone(),
                                    num[1].clone(),
                                    num[2].clone(),
                                    num[1].clone(),
                                    num[4].clone(),
                                    num[0].clone(),
                                    num[0].clone(),
                                    num[1].clone()],
                                  &[])
                  .is_err());

         */
    }

    #[test]
    fn test_turbo_plonk_circuit_2() {
        let mut cs = TurboPlonkConstraintSystem::new();
        let num: Vec<F> = (0..9).map(|x| F::from_u32(x as u32)).collect();

        // The circuit description:
        // 1. a \in {0, 1}
        // 2. a + b = c
        // 3. a + b + c + d = e
        // 4. b * c = f
        // 5. 0 <= e < 8
        // 6. 0 <= f < 8
        // The witness: [a, b, c, d, e, f] = [1, 2, 3, 1, 7, 6]
        let variables = vec![num[1], num[2], num[3], num[1], num[7], num[6]];
        cs.add_variables(&variables);
        cs.insert_boolean_gate(0);
        cs.insert_add_gate(0, 1, 2);
        cs.insert_lc_gate(&[0, 1, 2, 3], 4, num[1], num[1], num[1], num[1]);
        cs.insert_mul_gate(1, 2, 5);
        cs.range_check(4, 3);
        cs.range_check(5, 3);

        let twelve = num[8].add(&num[4]);
        // Good witness: [1, 2, 3, 1, 7, 6], e_binary_le = [1, 1, 1], f_binary_le = [0, 1, 1]
        let witness = cs.get_and_clear_witness();
        pnk!(cs.verify_witness(&witness[..], &[]));

        // Another good witness also satisfies the circuit:
        // [0, 2, 2, 1, 5, 4], e_binary_le = [1, 0, 1], f_binary_le = [0, 0, 1]
        let verify = cs.verify_witness(
            &[
                num[0], num[2], num[2], num[1], num[5], num[4], num[1], num[0], num[1],
                num[0], num[0], num[1],
            ],
            &[],
        );
        pnk!(verify);

        // Bad witness: a is not boolean
        assert!(cs
            .verify_witness(
                &[
                    num[2], num[0], num[2], num[1], num[5], num[0], num[1], num[0],
                    num[1], num[0], num[0], num[0]
                ],
                &[]
            )
            .is_err());
        // Bad witness: a + b != c
        assert!(cs
            .verify_witness(
                &[
                    num[1], num[1], num[1], num[2], num[5], num[1], num[1], num[0],
                    num[1], num[1], num[0], num[0]
                ],
                &[]
            )
            .is_err());
        // Bad witness: a + b + c + d != e
        assert!(cs
            .verify_witness(
                &[
                    num[1], num[1], num[2], num[2], num[5], num[2], num[1], num[0],
                    num[1], num[0], num[1], num[0]
                ],
                &[]
            )
            .is_err());
        // Bad witness: b * c != f
        assert!(cs
            .verify_witness(
                &[
                    num[1], num[1], num[2], num[2], num[6], num[1], num[0], num[1],
                    num[1], num[1], num[0], num[0]
                ],
                &[]
            )
            .is_err());
        // Bad witness: e >= 8, set e_binary_le = [1, 1, 1]
        assert!(cs
            .verify_witness(
                &[
                    num[1], num[2], num[3], num[2], num[8], num[6], num[1], num[1],
                    num[1], num[0], num[1], num[1]
                ],
                &[]
            )
            .is_err());
        // Bad witness: f >= 8, set f_binary_le = [1, 1, 1]
        assert!(cs
            .verify_witness(
                &[
                    num[0], num[3], num[4], num[0], num[7], twelve, num[1], num[1],
                    num[1], num[1], num[1], num[1]
                ],
                &[]
            )
            .is_err());
    }
}

#[cfg(test)]
mod turbo_plonk_proofs_test {
    use crate::commitments::kzg_poly_com::KZGCommitmentScheme;
    use crate::commitments::pcs::PolyComScheme;
    use crate::plonk::plonk_setup::preprocess_prover;
    use crate::plonk::protocol::prover::{prover, verifier};
    use crate::plonk::turbo_plonk_cs::rescue::State;
    use crate::plonk::turbo_plonk_cs::TurboPlonkConstraintSystem;
    use algebra::bls12_381::BLSScalar;
    use algebra::groups::{Group, One, Scalar, ScalarArithmetic, Zero};
    use algebra::jubjub::JubjubPoint;
    use merlin::Transcript;
    use rand_chacha::ChaChaRng;
    use rand_core::{CryptoRng, RngCore, SeedableRng};
    use std::str::FromStr;

    #[test]
    fn test_turbo_plonk_kzg() {
        let mut prng = ChaChaRng::from_seed([1u8; 32]);
        let pcs = KZGCommitmentScheme::new(20, &mut prng);
        test_turbo_plonk_with_constant_and_online_values(&pcs, &mut prng);
        test_turbo_plonk_arithmetic_gates(&pcs, &mut prng);
    }

    #[test]
    fn test_turbo_plonk_kzg_slow() {
        let mut prng = ChaChaRng::from_seed([1u8; 32]);
        let pcs = KZGCommitmentScheme::new(260, &mut prng);
        test_turbo_plonk_ecc_gates(&pcs, &mut prng);
        test_turbo_plonk_rescue_gates(&pcs, &mut prng);
    }

    fn test_turbo_plonk_with_constant_and_online_values<
        PCS: PolyComScheme,
        R: CryptoRng + RngCore,
    >(
        pcs: &PCS,
        prng: &mut R,
    ) {
        let one = PCS::Field::one();
        let two = one.add(&one);
        let three = two.add(&one);
        let four = three.add(&one);
        let seven = four.add(&three);
        let twenty_one = seven.mul(&three);
        let twenty_five = twenty_one.add(&four);

        // circuit (x_0 + y0) * (x_2 + 4) + x_0 * y1;
        // y0, y1 are online variables
        // witness (1 + 2) * (3 + 4) + 1 * 4 = 25
        let mut cs = TurboPlonkConstraintSystem::<PCS::Field>::new();
        cs.add_variables(&[
            one,
            two,
            three,
            four,
            three,
            seven,
            twenty_one,
            four,
            four,
            twenty_five,
        ]);
        cs.insert_add_gate(0, 1, 4);
        cs.insert_add_gate(2, 3, 5);
        cs.insert_mul_gate(4, 5, 6);
        cs.insert_mul_gate(0, 7, 8);
        cs.insert_add_gate(6, 8, 9);
        cs.insert_constant_gate(3, four);
        cs.prepare_io_variable(1);
        cs.prepare_io_variable(7);
        cs.pad();

        let mut online_vars = [two, four];
        let witness = cs.get_and_clear_witness();
        assert!(cs.verify_witness(&witness, &online_vars).is_ok());
        check_turbo_plonk_proof(pcs, prng, &cs, &witness, &online_vars);

        online_vars[0] = four;
        assert!(cs.verify_witness(&witness, &online_vars).is_err());
    }

    fn test_turbo_plonk_arithmetic_gates<PCS: PolyComScheme, R: CryptoRng + RngCore>(
        pcs: &PCS,
        prng: &mut R,
    ) {
        let mut cs = TurboPlonkConstraintSystem::new();
        let num: Vec<PCS::Field> =
            (0..9).map(|x| PCS::Field::from_u32(x as u32)).collect();

        // The circuit description:
        // 1. a \in {0, 1}
        // 2. c = add(a, b)
        // 3. d = mul(a, b)
        // 4. e = 2 * a + 3 * b + c + d
        // 5. 0 <= e < 16
        // The secret inputs: [a, b] = [1, 2]
        cs.new_variable(num[1]);
        cs.new_variable(num[2]);
        cs.insert_boolean_gate(0);
        let c_idx = cs.add(0, 1);
        let d_idx = cs.mul(0, 1);
        let e_idx =
            cs.linear_combine(&[0, 1, c_idx, d_idx], num[2], num[3], num[1], num[1]);
        cs.range_check(e_idx, 4);
        cs.pad();

        let witness = cs.get_and_clear_witness();
        assert!(cs.verify_witness(&witness[..], &[]).is_ok());
        check_turbo_plonk_proof(pcs, prng, &cs, &witness, &[]);
    }

    fn test_turbo_plonk_ecc_gates<
        PCS: PolyComScheme<Field = BLSScalar>,
        R: CryptoRng + RngCore,
    >(
        pcs: &PCS,
        prng: &mut R,
    ) {
        let mut cs = TurboPlonkConstraintSystem::new();

        // Compute secret scalar and public base point.
        let scalar_bytes: [u8; 32] = [
            47, 113, 87, 95, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let scalar = BLSScalar::from_bytes(&scalar_bytes).unwrap();
        let base_ext = JubjubPoint::get_base();

        // The circuit: P = [scalar] * G
        let scalar_var = cs.new_variable(scalar);
        cs.scalar_mul(base_ext, scalar_var, 64);
        cs.pad();
        let witness = cs.get_and_clear_witness();
        assert!(cs.verify_witness(&witness[..], &[]).is_ok());
        check_turbo_plonk_proof(pcs, prng, &cs, &witness, &[]);
    }

    fn test_turbo_plonk_rescue_gates<
        PCS: PolyComScheme<Field = BLSScalar>,
        R: CryptoRng + RngCore,
    >(
        pcs: &PCS,
        prng: &mut R,
    ) {
        let zero_vec = [PCS::Field::zero(); 4];
        let mut cs = TurboPlonkConstraintSystem::<PCS::Field>::new();
        // Prove the knowledge of hash pre-image.
        let input_state = State::new(zero_vec);
        let input_var = cs.new_hash_input_variable(input_state);
        let out_var = cs.rescue_hash(&input_var)[0];
        cs.prepare_io_variable(out_var);
        cs.pad();

        let online_vars = [PCS::Field::from_str(
      "52184923318241479436224725218017640784400243367974222506608059144773855444730",
    )
    .unwrap()];
        let witness = cs.get_and_clear_witness();
        assert!(cs.verify_witness(&witness, &online_vars).is_ok());
        check_turbo_plonk_proof(pcs, prng, &cs, &witness[..], &online_vars[..]);
    }

    fn check_turbo_plonk_proof<PCS: PolyComScheme, R: CryptoRng + RngCore>(
        pcs: &PCS,
        prng: &mut R,
        cs: &TurboPlonkConstraintSystem<PCS::Field>,
        witness: &[PCS::Field],
        online_vars: &[PCS::Field],
    ) {
        let common_seed = [0u8; 32];
        let prover_params = preprocess_prover(cs, pcs, common_seed).unwrap();
        let verifier_params_ref = &prover_params.verifier_params;

        let mut transcript = Transcript::new(b"TestTurboPlonk");
        let proof =
            prover(prng, &mut transcript, pcs, cs, &prover_params, witness).unwrap();

        let mut transcript = Transcript::new(b"TestTurboPlonk");
        assert!(verifier(
            &mut transcript,
            pcs,
            cs,
            verifier_params_ref,
            online_vars,
            &proof
        )
        .is_ok());
    }
}
