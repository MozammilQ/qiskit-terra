// This code is part of Qiskit.
//
// (C) Copyright IBM 2024
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

use crate::synthesis::linear::pmh::synth_pmh;
use ndarray::Array2;
use numpy::PyReadonlyArray2;
use pyo3::{prelude::*, types::PyList};
use qiskit_circuit::circuit_data::CircuitData;
use qiskit_circuit::operations::{Param, StandardGate};
use qiskit_circuit::Qubit;
use smallvec::{smallvec, SmallVec};
use std::f64::consts::PI;

type Instruction = (StandardGate, SmallVec<[Param; 3]>, SmallVec<[Qubit; 2]>);

struct InstructionIterator {
    s_cpy: Array2<u8>,
    state_cpy: Array2<u8>,
    rust_angles_cpy: Vec<String>,
    num_qubits: usize,
    qubit_idx: usize,
    index: usize,
}

impl InstructionIterator {
    fn new(s_cpy: Array2<u8>, state_cpy: Array2<u8>, rust_angles_cpy: Vec<String>) -> Self {
        let num_qubits = s_cpy.nrows();
        Self {
            s_cpy,
            state_cpy,
            rust_angles_cpy,
            num_qubits,
            qubit_idx: 0,
            index: 0,
        }
    }

    fn current_state(&self) -> (Array2<u8>, Vec<String>) {
        (self.s_cpy.clone(), self.rust_angles_cpy.clone())
    }
}

impl Iterator for InstructionIterator {
    type Item = Instruction;

    fn next(&mut self) -> Option<Instruction> {
        if self.qubit_idx >= self.num_qubits {
            return None;
        }

        if self.index < self.s_cpy.ncols() {
            let mut gate_instr: Option<Instruction> = None;
            let icnot = self.s_cpy.column(self.index).to_vec();
            self.index += 1;
            let target_state = self.state_cpy.row(self.qubit_idx).to_vec();

            if icnot == target_state {
                self.index -= 1;
                self.s_cpy.remove_index(numpy::ndarray::Axis(1), self.index);
                let angle = self.rust_angles_cpy.remove(self.index);

                gate_instr = Some(match angle.as_str() {
                    "t" => (
                        StandardGate::TGate,
                        smallvec![],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                    "tgd" => (
                        StandardGate::TdgGate,
                        smallvec![],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                    "s" => (
                        StandardGate::SGate,
                        smallvec![],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                    "sdg" => (
                        StandardGate::SdgGate,
                        smallvec![],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                    "z" => (
                        StandardGate::ZGate,
                        smallvec![],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                    angles_in_pi => (
                        StandardGate::PhaseGate,
                        smallvec![Param::Float((angles_in_pi.parse::<f64>().ok()?) % PI)],
                        smallvec![Qubit(self.qubit_idx as u32)],
                    ),
                });
            }
            if gate_instr.is_none() {
                self.next()
            } else {
                gate_instr
            }
        } else {
            self.qubit_idx += 1;
            self.index = 0;
            self.next()
        }
    }
}

/// This function implements a Gray-code inspired algorithm of synthesizing a circuit
/// over CNOT and phase-gates with minimal-CNOT for a given phase-polynomial.
/// The algorithm is described as "Gray-Synth" algorithm in Algorithm-1, page 12
/// of paper "https://arxiv.org/abs/1712.01859".
#[pyfunction]
#[pyo3(signature = (cnots, angles, section_size=2))]
pub fn synth_cnot_phase_aam(
    py: Python,
    cnots: PyReadonlyArray2<u8>,
    angles: &Bound<PyList>,
    section_size: Option<i64>,
) -> PyResult<CircuitData> {
    let s = cnots.as_array().to_owned();
    let num_qubits = s.nrows();
    let mut instructions = vec![];

    let rust_angles: Vec<String> = angles
        .iter()
        .filter_map(|data| data.extract::<String>().ok())
        .collect();
    let mut state = Array2::<u8>::eye(num_qubits);

    let mut instr_iter = InstructionIterator::new(s.clone(), state.clone(), rust_angles);

    let new_iter = std::iter::from_fn(|| instr_iter.next());
    let mut ins: Vec<Instruction> = new_iter.collect::<Vec<Instruction>>();
    let (mut s_cpy, mut rust_angles) = instr_iter.current_state();

    instructions.append(&mut ins);

    let epsilon: usize = num_qubits;
    let mut q = vec![(s, (0..num_qubits).collect::<Vec<usize>>(), epsilon)];

    while !q.is_empty() {
        let (mut _s, mut _i, mut _ep) = q.pop().unwrap();

        if _s.is_empty() {
            continue;
        }

        if _ep < num_qubits {
            let mut condition = true;
            while condition {
                condition = false;

                for _j in 0..num_qubits {
                    if (_j != _ep) && (_s.row(_j).sum() as usize == _s.row(_j).len()) {
                        condition = true;
                        instructions.push((
                            StandardGate::CXGate,
                            smallvec![],
                            smallvec![Qubit(_j as u32), Qubit(_ep as u32)],
                        ));

                        for _k in 0..state.ncols() {
                            state[(_ep, _k)] ^= state[(_j, _k)];
                        }

                        let mut index = 0_usize;
                        let mut swtch: bool = true;
                        while index < s_cpy.ncols() {
                            let icnot = s_cpy.column(index).to_vec();
                            if icnot == state.row(_ep).to_vec() {
                                match rust_angles.remove(index) {
                                    gate if gate == "t" => instructions.push((
                                        StandardGate::TGate,
                                        smallvec![],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                    gate if gate == "tdg" => instructions.push((
                                        StandardGate::TdgGate,
                                        smallvec![],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                    gate if gate == "s" => instructions.push((
                                        StandardGate::SGate,
                                        smallvec![],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                    gate if gate == "sdg" => instructions.push((
                                        StandardGate::SdgGate,
                                        smallvec![],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                    gate if gate == "z" => instructions.push((
                                        StandardGate::ZGate,
                                        smallvec![],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                    angles_in_pi => instructions.push((
                                        StandardGate::PhaseGate,
                                        smallvec![Param::Float(
                                            (angles_in_pi.parse::<f64>()?) % PI
                                        )],
                                        smallvec![Qubit(_ep as u32)],
                                    )),
                                };
                                s_cpy.remove_index(numpy::ndarray::Axis(1), index);
                                if index == s_cpy.ncols() {
                                    break;
                                }
                                if index == 0 {
                                    swtch = false;
                                } else {
                                    index -= 1;
                                }
                            }
                            if swtch {
                                index += 1;
                            } else {
                                swtch = true;
                            }
                        }

                        q.push((_s, _i, _ep));
                        let mut unique_q = vec![];
                        for data in q.into_iter() {
                            if !unique_q.contains(&data) {
                                unique_q.push(data);
                            }
                        }

                        q = unique_q;

                        for data in &mut q {
                            let (ref mut _temp_s, _, _) = data;

                            if _temp_s.is_empty() {
                                continue;
                            }

                            for idx in 0.._temp_s.row(_j).len() {
                                _temp_s[(_j, idx)] ^= _temp_s[(_ep, idx)];
                            }
                        }

                        (_s, _i, _ep) = q.pop().unwrap();
                    }
                }
            }
        }

        if _i.is_empty() {
            continue;
        }

        let maxes: Vec<usize> = _s
            .axis_iter(numpy::ndarray::Axis(0))
            .map(|row| {
                std::cmp::max(
                    row.iter().filter(|&&x| x == 0).count(),
                    row.iter().filter(|&&x| x == 1).count(),
                )
            })
            .collect();

        let maxes2: Vec<usize> = _i.iter().map(|&_i_idx| maxes[_i_idx]).collect();

        let _temp_argmax = maxes2
            .iter()
            .enumerate()
            .max_by(|(_, x), (_, y)| x.cmp(y))
            .map(|(idx, _)| idx)
            .unwrap();

        let _j = _i[_temp_argmax];

        let mut cnots0_t = vec![];
        let mut cnots1_t = vec![];

        let mut cnots0_t_shape = (0_usize, _s.column(0).len());
        let mut cnots1_t_shape = (0_usize, 0_usize);
        cnots1_t_shape.1 = cnots0_t_shape.1;
        for cols in _s.columns() {
            if cols[_j] == 0 {
                cnots0_t_shape.0 += 1;
                cnots0_t.append(&mut cols.to_vec());
            } else if cols[_j] == 1 {
                cnots1_t_shape.0 += 1;
                cnots1_t.append(&mut cols.to_vec());
            }
        }

        let cnots0 =
            Array2::from_shape_vec((cnots0_t_shape.0, cnots0_t_shape.1), cnots0_t).unwrap();
        let cnots1 =
            Array2::from_shape_vec((cnots1_t_shape.0, cnots1_t_shape.1), cnots1_t).unwrap();

        let cnots0 = cnots0.reversed_axes().to_owned();
        let cnots1 = cnots1.reversed_axes().to_owned();

        if _ep == num_qubits {
            q.push((
                cnots1,
                _i.clone().into_iter().filter(|&x| x != _j).collect(),
                _j,
            ));
        } else {
            q.push((
                cnots1,
                _i.clone().into_iter().filter(|&x| x != _j).collect(),
                _ep,
            ));
        }

        q.push((
            cnots0,
            _i.clone().into_iter().filter(|&x| x != _j).collect(),
            _ep,
        ));
    }

    let state_bool = state.mapv(|x| x != 0);
    let mut instrs = synth_pmh(state_bool, section_size)
        .into_iter()
        .rev()
        .collect();
    instructions.append(&mut instrs);
    CircuitData::from_standard_gates(py, num_qubits as u32, instructions, Param::Float(0.0))
}